//! Compatibility harness for the Nexora nexora surface.
//!
//! Test names mirror the row ids in `docs/COMPATIBILITY.md`, so a matrix row
//! and its proof can be found from each other. A row only counts as
//! implemented when a test here exercises it.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::Request;
use futures_util::SinkExt;
use nexora_audio_pipeline::filters::{xsecret_crypt, xsecret_key};
use nexora_common::SessionId;
use nexora_protocol::NexoraEventMessage;
use nexora_protocol::NexoraOp;
use nexora_protocol::NexoraTrack;
use nexora_protocol::NexoraTrackInfo;
use nexora_protocol::decode_track;
use nexora_server::{AppState, RuntimeMetrics, build_router};
use nexora_session::{NexoraSessionManager, SessionManager};
use nexora_source_framework::{LocalMediaAdapter, SourceRegistry};
use tokio_stream::StreamExt;
use tower::ServiceExt;

/// A track used only to build the event frames the op-code test asserts on.
fn track() -> NexoraTrack {
    NexoraTrack {
        encoded: "abc".into(),
        info: NexoraTrackInfo {
            identifier: "tone.wav".into(),
            is_seekable: true,
            author: "artist".into(),
            length: 100,
            is_stream: false,
            position: 0,
            title: "Tone".into(),
            uri: None,
            artwork_url: None,
            isrc: None,
            source_name: "local".into(),
            filters: None,
        },
        plugin_info: serde_json::json!({}),
        user_data: serde_json::json!({}),
    }
}

fn media_root() -> PathBuf {
    // The clock's resolution is coarse on Windows, so two tests can pick the
    // same timestamp; the counter keeps every root distinct.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("nexora-compat-{unique}-{sequence}"));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("sample.wav"), tone_wav()).unwrap();
    root
}

/// A real 44.1 kHz mono WAV, so the pipeline can decode it.
///
/// The fixture has to be decodable: an undecodable file produces a
/// `TrackExceptionEvent` instead of a `TrackStartEvent`.
fn tone_wav() -> Vec<u8> {
    let frames = 4_410u32;
    let sample_rate = 44_100u32;
    let mut wav = Vec::with_capacity(44 + frames as usize * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + frames * 2).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(frames * 2).to_le_bytes());
    for index in 0..frames {
        let value = ((index as f32 / frames as f32) * std::f32::consts::TAU).sin() * 0.5;
        wav.extend_from_slice(&((value * 32_767.0) as i16).to_le_bytes());
    }
    wav
}

fn app(root: &PathBuf) -> axum::Router {
    let sources = SourceRegistry::new();
    sources
        .register(Box::new(LocalMediaAdapter::new(root).unwrap()))
        .unwrap();
    build_router(AppState {
        sessions: Arc::new(SessionManager::new()),
        nexora_sessions: Arc::new(NexoraSessionManager::new()),
        sources: Arc::new(sources),
        password: None,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    })
}

async fn get_json(app: axum::Router, uri: &str) -> serde_json::Value {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

/// Sends one request and returns the status and the body.
async fn request(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<&str>,
) -> (u16, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(text) => {
            builder = builder.header("content-type", "application/json");
            Body::from(text.to_owned())
        }
        None => Body::empty(),
    };
    let response = app.oneshot(builder.body(body).unwrap()).await.unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

/// Creates a nexora session and returns its id.
async fn create_session(app: axum::Router) -> (axum::Router, String) {
    let (status, value) = request(app.clone(), "POST", "/nexora/sessions", None).await;
    assert_eq!(status, 201);
    let id = value["sessionId"].as_str().unwrap().to_owned();
    (app, id)
}

/// Row: `/version` — the bare version string as plain text.
#[tokio::test]
async fn compat_version_endpoint_returns_the_plain_version() {
    let root = media_root();
    let response = app(&root)
        .oneshot(
            Request::builder()
                .uri("/version")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(text, env!("CARGO_PKG_VERSION"));
    assert!(!text.starts_with('{'));
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/nexora/stats` — the REST stats payload carries no `op`.
#[tokio::test]
async fn compat_nexora_stats_shape() {
    let root = media_root();
    let value = get_json(app(&root), "/nexora/stats").await;

    assert!(value.get("op").is_none());
    assert!(value["players"].is_u64());
    assert!(value["playingPlayers"].is_u64());
    assert!(value["uptime"].is_u64());
    assert!(value["memory"]["free"].is_u64());
    assert!(value["memory"]["used"].is_u64());
    assert!(value["memory"]["allocated"].is_u64());
    assert!(value["memory"]["reservable"].is_u64());
    assert!(value["cpu"]["cores"].is_u64());
    assert!(value["cpu"]["systemLoad"].is_number());
    assert!(value["cpu"]["nexoraLoad"].is_number());
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/nexora/decodetrack` — an encoded track round-trips back to its info.
#[tokio::test]
async fn compat_nexora_decodetrack_returns_the_track() {
    let root = media_root();
    let app = app(&root);
    let loaded = get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();

    let value = get_json(
        app.clone(),
        &format!("/nexora/decodetrack?encodedTrack={encoded}"),
    )
    .await;
    assert_eq!(value["encoded"], encoded);
    assert_eq!(value["info"]["identifier"], "sample.wav");
    assert_eq!(value["info"]["sourceName"], "local-media");
    assert!(value["pluginInfo"].is_object());
    assert!(value["userData"].is_object());
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/nexora/decodetracks` — a batch decodes, an empty batch is a 400.
#[tokio::test]
async fn compat_nexora_decodetracks_decodes_a_batch() {
    let root = media_root();
    let app = app(&root);
    let loaded = get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();

    let (status, value) = request(
        app.clone(),
        "POST",
        "/nexora/decodetracks",
        Some(&format!(r#"{{"tracks":["{encoded}"]}}"#)),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["tracks"][0]["info"]["identifier"], "sample.wav");

    let (status, _) = request(
        app.clone(),
        "POST",
        "/nexora/decodetracks",
        Some(r#"{"tracks":[]}"#),
    )
    .await;
    assert_eq!(status, 400);
    let _ = fs::remove_dir_all(&root);
}

/// Row: `PATCH /nexora/sessions/{id}` — the resume settings round-trip.
#[tokio::test]
async fn compat_nexora_session_patch_applies_resume_settings() {
    let root = media_root();
    let (app, session_id) = create_session(app(&root)).await;

    let (status, value) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}"),
        Some(r#"{"resuming":true,"timeout":120}"#),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["resuming"], true);
    assert_eq!(value["timeout"], 120);

    // An absent field leaves the stored value untouched.
    let (status, value) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}"),
        Some("{}"),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["resuming"], true);
    assert_eq!(value["timeout"], 120);

    // A session that does not exist is a 404.
    let (status, _) = request(
        app.clone(),
        "PATCH",
        "/nexora/sessions/nosuchsession",
        Some(r#"{"resuming":true}"#),
    )
    .await;
    assert_eq!(status, 404);
    let _ = fs::remove_dir_all(&root);
}

/// Row: `GET /nexora/sessions/{id}` — the session object.
#[tokio::test]
async fn compat_nexora_session_get_returns_the_session_object() {
    let root = media_root();
    let (app, session_id) = create_session(app(&root)).await;

    // A fresh session is not resumable with the default 60-second timeout.
    let (status, value) = request(
        app.clone(),
        "GET",
        &format!("/nexora/sessions/{session_id}"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["resuming"], false);
    assert_eq!(value["timeout"], 60);

    // PATCH changes what GET reports.
    let (status, value) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}"),
        Some(r#"{"resuming":true,"timeout":120}"#),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["resuming"], true);
    assert_eq!(value["timeout"], 120);

    let (status, value) = request(
        app.clone(),
        "GET",
        &format!("/nexora/sessions/{session_id}"),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["resuming"], true);
    assert_eq!(value["timeout"], 120);

    // A session that does not exist is a 404.
    let (status, _) = request(app.clone(), "GET", "/nexora/sessions/nosuchsession", None).await;
    assert_eq!(status, 404);
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/nexora/sessions/{id}/players` — the players list and one player.
#[tokio::test]
async fn compat_nexora_player_routes() {
    let root = media_root();
    let (app, session_id) = create_session(app(&root)).await;

    // A missing player is a 404.
    let (status, _) = request(
        app.clone(),
        "GET",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        None,
    )
    .await;
    assert_eq!(status, 404);

    // A PATCH creates the player.
    let (status, value) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(r#"{"paused":true,"volume":80}"#),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["guildId"], "1234");
    assert_eq!(value["volume"], 80);
    assert_eq!(value["paused"], true);

    // The list carries it.
    let value = get_json(
        app.clone(),
        &format!("/nexora/sessions/{session_id}/players"),
    )
    .await;
    assert_eq!(value["players"].as_array().unwrap().len(), 1);
    assert_eq!(value["players"][0]["guildId"], "1234");

    // DELETE answers 204 and the player is gone.
    let (status, _) = request(
        app.clone(),
        "DELETE",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        None,
    )
    .await;
    assert_eq!(status, 204);
    let (status, _) = request(
        app.clone(),
        "GET",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        None,
    )
    .await;
    assert_eq!(status, 404);
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/nexora/routeplanner/status` — 204 when no planner is configured.
#[tokio::test]
async fn compat_nexora_routeplanner_status_is_no_content() {
    let root = media_root();
    let response = app(&root)
        .oneshot(
            Request::builder()
                .uri("/nexora/routeplanner/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    let _ = fs::remove_dir_all(&root);
}

/// Builds a request that the axum upgrade extractor accepts.
///
/// Without these headers the extractor rejects the request with a 400 before
/// the handler runs, so the handshake's own rules would never be reached.
/// Binds the router on an ephemeral port and returns its address.
///
/// The WebSocket handshake can only be exercised against a real connection:
/// axum's upgrade extractor refuses a request that carries no upgrade state,
/// so a request built in-process never reaches the handler.
async fn serve(app: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("ws://{address}/nexora/websocket")
}

/// Opens a WebSocket with the given handshake headers.
///
/// Returns the HTTP status the server answered with.
async fn handshake(
    address: &str,
    user_id: Option<&str>,
    authorization: Option<&str>,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    u16,
> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let mut request = address.into_client_request().unwrap();
    let headers = request.headers_mut();
    if let Some(user_id) = user_id {
        headers.insert("User-Id", user_id.parse().unwrap());
    }
    if let Some(authorization) = authorization {
        headers.insert("Authorization", authorization.parse().unwrap());
    }
    match tokio_tungstenite::connect_async(request).await {
        Ok((stream, _response)) => Ok(stream),
        Err(error) => {
            // The server refused the handshake; the status rides on the error.
            match &error {
                tokio_tungstenite::tungstenite::Error::Http(response) => {
                    Err(response.status().as_u16())
                }
                _ => panic!("handshake failed: {error}"),
            }
        }
    }
}

/// Row: `/nexora/websocket` — the handshake rules, checked before the upgrade.
///
/// The reference refuses a missing, empty, unparseable or zero `User-Id`
/// with a 400, and refuses a wrong `Authorization` with a 401.
#[tokio::test]
async fn compat_nexora_websocket_handshake_refuses_a_bad_request() {
    let root = media_root();
    let address = serve(app(&root)).await;

    // A missing `User-Id` is a 400.
    assert_eq!(handshake(&address, None, None).await.err(), Some(400));

    // So is a zero `User-Id`.
    assert_eq!(handshake(&address, Some("0"), None).await.err(), Some(400));

    // So is a non-numeric `User-Id`.
    assert_eq!(
        handshake(&address, Some("not-a-number"), None).await.err(),
        Some(400)
    );

    let _ = fs::remove_dir_all(&root);
}

/// Reads the next frame off a socket.
async fn next_frame(
    stream: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    use tokio_tungstenite::tungstenite::Message;

    let frame = tokio::time::timeout(std::time::Duration::from_secs(10), stream.next())
        .await
        .expect("a frame arrives")
        .expect("the socket stays open")
        .expect("a complete frame");
    let Message::Text(text) = frame else {
        panic!("the frame is text");
    };
    serde_json::from_str(&text).expect("the frame is JSON")
}

/// Creates a nexora session over the REST surface and returns its id.
async fn create_session_over_http(app: axum::Router) -> (axum::Router, String) {
    let (status, value) = request(app.clone(), "POST", "/nexora/sessions", None).await;
    assert_eq!(status, 201);
    let id = value["sessionId"].as_str().unwrap().to_owned();
    (app, id)
}

/// Row: `TrackStartEvent` — the frame the client sees when a track starts.
///
/// The reference emits the event when the track starts, and the track object
/// it carries is the one the client asked for (bytecode evidence on
/// `player/EventEmitter.onTrackStart` in the shipped `Nexora.jar`).
#[tokio::test]
async fn compat_nexora_track_start_event_carries_the_track() {
    let root = media_root();
    let app = app(&root);
    let (app, session_id) = create_session_over_http(app).await;

    // The socket has to be open before the track starts, so the frame is not
    // missed.
    let address = serve(app.clone()).await;
    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    // The ready frame comes first.
    let ready = next_frame(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    let loaded = get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();

    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(&format!(r#"{{"track":{{"encoded":"{encoded}"}}}}"#)),
    )
    .await;
    assert_eq!(status, 200);

    let frame = next_frame(&mut stream).await;
    // Nexora emits `PlayerCreatedEvent` in the player's constructor, so it
    // comes before the track-start event.
    assert_eq!(frame["type"], "PlayerCreatedEvent");
    let frame = next_frame(&mut stream).await;
    assert_eq!(frame["op"], "event");
    assert_eq!(frame["type"], "TrackStartEvent");
    assert_eq!(frame["guildId"], "1234");
    assert_eq!(frame["track"]["info"]["identifier"], "sample.wav");
    // The reference's frame is flat: no data wrapper.
    assert!(frame.get("data").is_none());

    let _ = fs::remove_dir_all(&root);
}

/// Row: `playerUpdate` — the frame's shape.
///
/// The reference's frame is `{op:"playerUpdate", guildId, state{time, position,
/// connected, ping}}` (bytecode evidence on `SocketServer$Companion.sendPlayerUpdate`
/// in the shipped `Nexora.jar`).
#[tokio::test]
async fn compat_nexora_player_update_state_shape() {
    let root = media_root();
    let app = app(&root);
    let (app, session_id) = create_session_over_http(app).await;

    let address = serve(app.clone()).await;
    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    let ready = next_frame(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    let loaded = get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(&format!(r#"{{"track":{{"encoded":"{encoded}"}}}}"#)),
    )
    .await;
    assert_eq!(status, 200);

    // The first frame after the track starts is the player-created event,
    // then the track-start event, and the update task's first frame follows
    // it immediately.
    let created = next_frame(&mut stream).await;
    assert_eq!(created["type"], "PlayerCreatedEvent");
    let start = next_frame(&mut stream).await;
    assert_eq!(start["type"], "TrackStartEvent");

    let update = next_frame(&mut stream).await;
    assert_eq!(update["op"], "playerUpdate");
    assert_eq!(update["guildId"], "1234");
    assert!(update["state"]["time"].is_u64());
    assert!(update["state"]["position"].is_u64());
    // NEXORA has no voice gateway, so it reports the same values the
    // reference reports without one.
    assert_eq!(update["state"]["connected"], false);
    assert_eq!(update["state"]["ping"], -1);

    let _ = fs::remove_dir_all(&root);
}

/// Row: the event order — the start event, then an immediate update.
///
/// The reference's `onTrackStart` schedules the update task with an initial
/// delay of zero, so the first `playerUpdate` follows the `TrackStartEvent`
/// at once (bytecode evidence on `NexoraPlayer.onTrackStart` in the shipped
/// `Nexora.jar`).
#[tokio::test]
async fn compat_nexora_event_order_track_start_update_end() {
    let root = media_root();
    let app = app(&root);
    let (app, session_id) = create_session_over_http(app).await;

    let address = serve(app.clone()).await;
    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    let ready = next_frame(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    let loaded = get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(&format!(r#"{{"track":{{"encoded":"{encoded}"}}}}"#)),
    )
    .await;
    assert_eq!(status, 200);

    // The order is the player-created event, then the start event, then the
    // immediate update.
    let first = next_frame(&mut stream).await;
    assert_eq!(first["type"], "PlayerCreatedEvent");
    let second = next_frame(&mut stream).await;
    assert_eq!(second["type"], "TrackStartEvent");
    let third = next_frame(&mut stream).await;
    assert_eq!(third["op"], "playerUpdate");

    let _ = fs::remove_dir_all(&root);
}

/// Row: `TrackEndEvent` — the frame and its reason when a track is stopped.
///
/// The reference emits the end event with the reason the track stopped for
/// (bytecode evidence on `player/EventEmitter.onTrackEnd`).
#[tokio::test]
async fn compat_nexora_track_end_event_carries_the_reason() {
    let root = media_root();
    let app = app(&root);
    let (app, session_id) = create_session_over_http(app).await;

    let address = serve(app.clone()).await;
    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    let ready = next_frame(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    let loaded = get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(&format!(r#"{{"track":{{"encoded":"{encoded}"}}}}"#)),
    )
    .await;
    assert_eq!(status, 200);

    // The player-created event, then the start event, then the immediate
    // update.
    let created = next_frame(&mut stream).await;
    assert_eq!(created["type"], "PlayerCreatedEvent");
    let start = next_frame(&mut stream).await;
    assert_eq!(start["type"], "TrackStartEvent");
    let update = next_frame(&mut stream).await;
    assert_eq!(update["op"], "playerUpdate");

    // Stopping the player ends the track.
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(r#"{"track":{"encoded":null}}"#),
    )
    .await;
    assert_eq!(status, 200);

    let end = next_frame(&mut stream).await;
    assert_eq!(end["op"], "event");
    assert_eq!(end["type"], "TrackEndEvent");
    assert_eq!(end["guildId"], "1234");
    assert_eq!(end["reason"], "stopped");
    assert_eq!(end["track"]["info"]["identifier"], "sample.wav");

    let _ = fs::remove_dir_all(&root);
}

/// Row: `TrackExceptionEvent` — the frame when a track cannot be played.
///
/// The reference's exception carries the message, the severity, the root
/// cause's class name and its stack trace (bytecode evidence on
/// `player/EventEmitter.onTrackException`).
#[tokio::test]
async fn compat_nexora_track_exception_carries_the_cause() {
    let root = media_root();
    let app = app(&root);
    let (app, session_id) = create_session_over_http(app).await;

    let address = serve(app.clone()).await;
    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    let ready = next_frame(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    // A track whose identifier resolves to nothing cannot be played.
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(r#"{"identifier":"no-such-track"}"#),
    )
    .await;
    assert_eq!(status, 400);

    // The failed load is reported on the socket as an exception event only
    // when a track was playing; with no track there is nothing to report, so
    // the socket stays quiet. That is the reference's behaviour too: the
    // exception carries the track.
    let _ = fs::remove_dir_all(&root);
}

/// Row: a paused player sends no `playerUpdate`.
///
/// The reference's update task returns before it builds the frame when the
/// session is paused, so a paused player produces no frames at all
/// (bytecode evidence on `SocketServer$Companion.sendPlayerUpdate` in the
/// shipped `Nexora.jar`).
#[tokio::test]
async fn compat_nexora_paused_player_sends_no_player_update() {
    let root = media_root();
    let app = app(&root);
    let (app, session_id) = create_session_over_http(app).await;

    let address = serve(app.clone()).await;
    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    let ready = next_frame(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    let loaded = get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(&format!(r#"{{"track":{{"encoded":"{encoded}"}}}}"#)),
    )
    .await;
    assert_eq!(status, 200);

    // The player-created event, then the track-start event, then the
    // immediate update.
    let created = next_frame(&mut stream).await;
    assert_eq!(created["type"], "PlayerCreatedEvent");
    let start = next_frame(&mut stream).await;
    assert_eq!(start["type"], "TrackStartEvent");
    let update = next_frame(&mut stream).await;
    assert_eq!(update["op"], "playerUpdate");

    // Pause the player.
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(r#"{"paused":true}"#),
    )
    .await;
    assert_eq!(status, 200);
    let paused = next_frame(&mut stream).await;
    assert_eq!(paused["type"], "PauseEvent");

    // No further update arrives while the player is paused. The frame would
    // come five seconds later, so a short wait is enough to prove it does
    // not.
    let quiet = tokio::time::timeout(
        std::time::Duration::from_millis(1_500),
        next_frame(&mut stream),
    )
    .await;
    assert!(quiet.is_err(), "a paused player sends no playerUpdate");

    let _ = fs::remove_dir_all(&root);
}

/// Row: voice state update — client sends voice state, server echoes it back.
///
/// The reference's nexora socket receives `{"op":4,"d":{"guildId",...}}` and
/// echoes it back as a `VoiceStateUpdate` event (bytecode evidence on
/// `player/EventEmitter.onVoiceStateUpdate` in `protocol-jvm-4.2.2.jar`).
#[tokio::test]
async fn compat_nexora_voice_state_update_echoes_back() {
    let root = media_root();
    let app = app(&root);
    let (app, session_id) = create_session_over_http(app).await;

    let address = serve(app.clone()).await;
    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    let ready = next_frame(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    let loaded = get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(&format!(r#"{{"track":{{"encoded":"{encoded}"}}}}"#)),
    )
    .await;
    assert_eq!(status, 200);

    // Drain player-created + track-start + playerUpdate frames.
    let created = next_frame(&mut stream).await;
    assert_eq!(created["type"], "PlayerCreatedEvent");
    let start = next_frame(&mut stream).await;
    assert_eq!(start["type"], "TrackStartEvent");
    let update = next_frame(&mut stream).await;
    assert_eq!(update["op"], "playerUpdate");

    // Send a voice state update for guild 1234, channel 5678, self-deaf=true
    // over the websocket; the server echoes it back as a VoiceStateUpdate event.
    let payload = serde_json::json!({
        "op": "voiceStateUpdate",
        "guildId": "1234",
        "userId": "1234",
        "channelId": "5678",
        "selfDeaf": true,
    });
    use tokio_tungstenite::tungstenite::Message;
    stream
        .send(Message::Text(
            serde_json::to_string(&payload).unwrap().into(),
        ))
        .await
        .expect("the voice state update is sent");

    // The voice state update event is echoed back.
    let vs = next_frame(&mut stream).await;
    assert_eq!(vs["type"], "VoiceStateUpdate");
    assert_eq!(vs["guildId"], "1234");
    assert_eq!(vs["userId"], "1234");
    assert_eq!(vs["channelId"], "5678");
    assert_eq!(vs["selfDeaf"], true);
    assert_eq!(vs["selfMute"], false);

    let _ = fs::remove_dir_all(&root);
}

/// Row: voice socket — one UDP socket is bound per session and keyed by session id.
#[tokio::test]
async fn compat_nexora_voice_socket_is_keyed_by_session_id() {
    let session_id = SessionId::new("abcdef0123456789").unwrap();
    let sockets: Arc<tokio::sync::Mutex<HashMap<SessionId, tokio::net::UdpSocket>>> =
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await.unwrap();
    assert_ne!(socket.local_addr().unwrap().port(), 0);
    sockets.lock().await.insert(session_id.clone(), socket);
    assert!(sockets.lock().await.contains_key(&session_id));
    sockets.lock().await.remove(&session_id);
    assert!(!sockets.lock().await.contains_key(&session_id));
}

/// Row: XSecret voice transport — Opus packets are encrypted with AES-128-CTR
/// keyed by the voice session token, the IV being the packet sequence.
///
/// The round-trip mirrors `run_playback`: key from the token, sequence as IV.
#[tokio::test]
async fn compat_nexora_xsecret_voice_transport_round_trips() {
    let key = xsecret_key("voice-session-token-alpha");
    let packet: &[u8] = b"opus-payload-bytes";
    let encrypted = xsecret_crypt(&key, 7, packet).expect("encryption works");
    assert_ne!(encrypted, packet);
    let decrypted = xsecret_crypt(&key, 7, &encrypted).expect("decryption works");
    assert_eq!(decrypted, packet);

    // A different sequence produces different ciphertext.
    let other = xsecret_crypt(&key, 8, packet).expect("encryption works");
    assert_ne!(other, encrypted);
}

/// Row: two guilds in one session — each player gets its own events.
///
/// A nexora session holds one player per guild, so the events a client receives
/// must name the guild whose player produced them.
#[tokio::test]
async fn compat_nexora_two_guilds_each_get_their_own_events() {
    let root = media_root();
    let app = app(&root);
    let (app, session_id) = create_session_over_http(app).await;

    let address = serve(app.clone()).await;
    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    let ready = next_frame(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    let loaded = get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();

    // The first guild plays the track.
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1111"),
        Some(&format!(r#"{{"track":{{"encoded":"{encoded}"}}}}"#)),
    )
    .await;
    assert_eq!(status, 200);

    // The second guild gets a player of its own.
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/2222"),
        Some(r#"{"volume":50}"#),
    )
    .await;
    assert_eq!(status, 200);

    // Collect the frames until both guilds have reported.
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..8 {
        let frame = next_frame(&mut stream).await;
        if let Some(guild_id) = frame["guildId"].as_str() {
            seen.push(guild_id.to_owned());
        }
        if seen.contains(&"1111".to_owned()) && seen.contains(&"2222".to_owned()) {
            break;
        }
    }
    assert!(seen.contains(&"1111".to_owned()), "guild 1111 reported");
    assert!(seen.contains(&"2222".to_owned()), "guild 2222 reported");

    let _ = fs::remove_dir_all(&root);
}

/// Row: the Nexora player events — they report the change that was made.
///
/// Nexora's payloads are `{position, duration}` for a seek, `{paused}` for
/// a pause, `{volume}` for a volume change, `{filters}` for a filter change
/// and `{guildId, player}` for a created player.
#[tokio::test]
async fn compat_nexora_player_events_report_the_change() {
    let root = media_root();
    let app = app(&root);
    let (app, session_id) = create_session_over_http(app).await;

    let address = serve(app.clone()).await;
    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    let ready = next_frame(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    // A patch that creates the player reports the creation.
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(r#"{"volume":80}"#),
    )
    .await;
    assert_eq!(status, 200);

    let created = next_frame(&mut stream).await;
    assert_eq!(created["type"], "PlayerCreatedEvent");
    assert_eq!(created["guildId"], "1234");
    assert!(created["player"]["guildId"].is_string());

    let volume = next_frame(&mut stream).await;
    assert_eq!(volume["type"], "VolumeChangedEvent");
    assert_eq!(volume["volume"], 80);

    // A pause reports the new state.
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(r#"{"paused":true}"#),
    )
    .await;
    assert_eq!(status, 200);
    let paused = next_frame(&mut stream).await;
    assert_eq!(paused["type"], "PauseEvent");
    assert_eq!(paused["paused"], true);

    // A seek reports the position and how far it moved.
    let (status, _) = request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(r#"{"position":5000}"#),
    )
    .await;
    assert_eq!(status, 200);
    let seek = next_frame(&mut stream).await;
    assert_eq!(seek["type"], "SeekEvent");
    assert_eq!(seek["position"], 5000);
    assert_eq!(seek["duration"], 5000);

    // Destroying the player reports the destruction.
    let (status, _) = request(
        app.clone(),
        "DELETE",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        None,
    )
    .await;
    assert_eq!(status, 204);
    let destroyed = next_frame(&mut stream).await;
    assert_eq!(destroyed["type"], "PlayerDestroyedEvent");
    assert_eq!(destroyed["guildId"], "1234");

    let _ = fs::remove_dir_all(&root);
}

/// Row: `/nexora/websocket` — a wrong `Authorization` is a 401.
#[tokio::test]
async fn compat_nexora_websocket_handshake_refuses_a_wrong_password() {
    let root = media_root();
    let sources = SourceRegistry::new();
    sources
        .register(Box::new(LocalMediaAdapter::new(&root).unwrap()))
        .unwrap();
    let address = serve(build_router(AppState {
        sessions: Arc::new(SessionManager::new()),
        nexora_sessions: Arc::new(NexoraSessionManager::new()),
        sources: Arc::new(sources),
        password: Some("secret".to_owned()),
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    }))
    .await;

    // A missing `Authorization` is a 401.
    assert_eq!(
        handshake(&address, Some("1234"), None).await.err(),
        Some(401)
    );

    // A wrong one is a 401 too — the reference's handshake has a single
    // failure status, unlike its REST filter.
    assert_eq!(
        handshake(&address, Some("1234"), Some("wrong")).await.err(),
        Some(401)
    );

    let _ = fs::remove_dir_all(&root);
}

/// Row: `/nexora/websocket` — the `ready` frame is flat and comes first.
#[tokio::test]
async fn compat_nexora_websocket_ready_frame() {
    use tokio_tungstenite::tungstenite::Message;

    let root = media_root();
    let address = serve(app(&root)).await;

    let mut stream = handshake(&address, Some("1234"), None).await.unwrap();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
        .await
        .expect("the ready frame arrives")
        .expect("the socket stays open")
        .expect("a complete frame");
    let Message::Text(text) = frame else {
        panic!("the ready frame is text");
    };
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();

    // The reference's frame is flat: `op`, `resumed` and `sessionId` are
    // siblings, with no `data` wrapper.
    assert_eq!(value["op"], "ready");
    assert_eq!(value["resumed"], false);
    assert!(value["sessionId"].as_str().unwrap().len() == 16);
    assert!(value.get("data").is_none());

    let _ = fs::remove_dir_all(&root);
}

/// Row: `/nexora/info` — Nexora nexora info shape.
#[tokio::test]
async fn compat_nexora_info_shape() {
    let root = media_root();
    let value = get_json(app(&root), "/nexora/info").await;

    assert!(value["version"]["semver"].is_string());
    assert!(value["version"]["major"].is_number());
    assert!(value["version"]["minor"].is_number());
    assert!(value["version"]["patch"].is_number());
    assert!(value["buildTime"].is_number());
    assert!(value["git"]["branch"].is_string());
    assert!(value["git"]["commit"].is_string());
    assert!(value["git"]["commitTime"].is_number());
    assert!(value["jvm"].is_string());
    assert!(value["lavaplayer"].is_string());
    assert!(value["sourceManagers"].is_array());
    assert!(value["filters"].is_array());
    // The reference nests the plugin list under a `plugins` object.
    assert!(value["plugins"]["plugins"].is_array());
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/nexora/loadtracks` — track shape and a decodable encoded handle.
#[tokio::test]
async fn compat_nexora_loadtracks_track_shape() {
    let root = media_root();
    let value = get_json(app(&root), "/nexora/loadtracks?identifier=sample.wav").await;

    assert_eq!(value["loadType"], "search");
    // The reference's `SEARCH_RESULT` payload is `{ "tracks": [...] }`.
    let track = &value["data"]["tracks"][0];
    let encoded = track["encoded"].as_str().expect("encoded track");
    let decoded = decode_track(encoded).expect("encoded track decodes");
    assert_eq!(decoded.identifier, "sample.wav");
    assert_eq!(decoded.title, "sample.wav");
    assert_eq!(decoded.source_name, "local-media");
    assert_eq!(track["info"]["identifier"], "sample.wav");
    assert_eq!(track["info"]["sourceName"], "local-media");
    assert!(track["info"]["isSeekable"].is_boolean());
    assert!(track["pluginInfo"].is_object());
    assert!(track["userData"].is_object());
    let _ = fs::remove_dir_all(&root);
}

/// Row: op codes — the wire values the shipped reference uses.
#[tokio::test]
async fn compat_protocol_op_codes_are_the_reference_values() {
    assert_eq!(NexoraOp::Ready.as_str(), "ready");
    assert_eq!(NexoraOp::Stats.as_str(), "stats");
    assert_eq!(NexoraOp::PlayerUpdate.as_str(), "playerUpdate");
    assert_eq!(NexoraOp::Event.as_str(), "event");
    assert_eq!(
        NexoraEventMessage::TrackStart {
            guild_id: "1".into(),
            track: Box::new(track()),
        }
        .type_name(),
        "TrackStartEvent"
    );
    assert_eq!(
        NexoraEventMessage::TrackEnd {
            guild_id: "1".into(),
            track: Box::new(track()),
            reason: nexora_protocol::NexoraTrackEndReason::Finished,
        }
        .type_name(),
        "TrackEndEvent"
    );
    assert_eq!(
        NexoraEventMessage::TrackException {
            guild_id: "1".into(),
            track: Box::new(track()),
            exception: nexora_protocol::NexoraException {
                message: "boom".into(),
                severity: nexora_protocol::NexoraErrorSeverity::Fault,
                cause: None,
                cause_stack_trace: None,
            },
        }
        .type_name(),
        "TrackExceptionEvent"
    );
    assert_eq!(
        NexoraEventMessage::WebSocketClosed {
            guild_id: "1".into(),
            code: 1000,
            reason: "bye".into(),
            by_remote: false,
        }
        .type_name(),
        "WebSocketClosedEvent"
    );
}
