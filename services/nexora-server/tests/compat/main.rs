//! Compatibility harness for the Lavalink v4 surface.
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
use nexora_protocol::V4EventMessage;
use nexora_protocol::V4Op;
use nexora_protocol::V4Track;
use nexora_protocol::V4TrackInfo;
use nexora_protocol::decode_track;
use nexora_server::{AppState, RuntimeMetrics, build_router};
use nexora_session::{SessionManager, V4SessionManager};
use nexora_source_framework::{LocalMediaAdapter, SourceRegistry};
use tokio_stream::StreamExt;
use tower::ServiceExt;

/// A track used only to build the event frames the op-code test asserts on.
fn track() -> V4Track {
    V4Track {
        encoded: "abc".into(),
        info: V4TrackInfo {
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
    fs::write(root.join("sample.wav"), b"not-a-real-wav").unwrap();
    root
}

fn app(root: &PathBuf) -> axum::Router {
    let sources = SourceRegistry::new();
    sources
        .register(Box::new(LocalMediaAdapter::new(root).unwrap()))
        .unwrap();
    build_router(AppState {
        sessions: Arc::new(SessionManager::new()),
        v4_sessions: Arc::new(V4SessionManager::new()),
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

/// Creates a v4 session and returns its id.
async fn create_session(app: axum::Router) -> (axum::Router, String) {
    let (status, value) = request(app.clone(), "POST", "/v4/sessions", None).await;
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

/// Row: `/v4/stats` — the REST stats payload carries no `op`.
#[tokio::test]
async fn compat_v4_stats_shape() {
    let root = media_root();
    let value = get_json(app(&root), "/v4/stats").await;

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
    assert!(value["cpu"]["lavalinkLoad"].is_number());
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/v4/decodetrack` — an encoded track round-trips back to its info.
#[tokio::test]
async fn compat_v4_decodetrack_returns_the_track() {
    let root = media_root();
    let app = app(&root);
    let loaded = get_json(app.clone(), "/v4/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();

    let value = get_json(
        app.clone(),
        &format!("/v4/decodetrack?encodedTrack={encoded}"),
    )
    .await;
    assert_eq!(value["encoded"], encoded);
    assert_eq!(value["info"]["identifier"], "sample.wav");
    assert_eq!(value["info"]["sourceName"], "local-media");
    assert!(value["pluginInfo"].is_object());
    assert!(value["userData"].is_object());
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/v4/decodetracks` — a batch decodes, an empty batch is a 400.
#[tokio::test]
async fn compat_v4_decodetracks_decodes_a_batch() {
    let root = media_root();
    let app = app(&root);
    let loaded = get_json(app.clone(), "/v4/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();

    let (status, value) = request(
        app.clone(),
        "POST",
        "/v4/decodetracks",
        Some(&format!(r#"{{"tracks":["{encoded}"]}}"#)),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["tracks"][0]["info"]["identifier"], "sample.wav");

    let (status, _) = request(
        app.clone(),
        "POST",
        "/v4/decodetracks",
        Some(r#"{"tracks":[]}"#),
    )
    .await;
    assert_eq!(status, 400);
    let _ = fs::remove_dir_all(&root);
}

/// Row: `PATCH /v4/sessions/{id}` — the resume settings round-trip.
#[tokio::test]
async fn compat_v4_session_patch_applies_resume_settings() {
    let root = media_root();
    let (app, session_id) = create_session(app(&root)).await;

    let (status, value) = request(
        app.clone(),
        "PATCH",
        &format!("/v4/sessions/{session_id}"),
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
        &format!("/v4/sessions/{session_id}"),
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
        "/v4/sessions/nosuchsession",
        Some(r#"{"resuming":true}"#),
    )
    .await;
    assert_eq!(status, 404);
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/v4/sessions/{id}/players` — the players list and one player.
#[tokio::test]
async fn compat_v4_player_routes() {
    let root = media_root();
    let (app, session_id) = create_session(app(&root)).await;

    // A missing player is a 404.
    let (status, _) = request(
        app.clone(),
        "GET",
        &format!("/v4/sessions/{session_id}/players/1234"),
        None,
    )
    .await;
    assert_eq!(status, 404);

    // A PATCH creates the player.
    let (status, value) = request(
        app.clone(),
        "PATCH",
        &format!("/v4/sessions/{session_id}/players/1234"),
        Some(r#"{"paused":true,"volume":80}"#),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["guildId"], "1234");
    assert_eq!(value["volume"], 80);
    assert_eq!(value["paused"], true);

    // The list carries it.
    let value = get_json(app.clone(), &format!("/v4/sessions/{session_id}/players")).await;
    assert_eq!(value["players"].as_array().unwrap().len(), 1);
    assert_eq!(value["players"][0]["guildId"], "1234");

    // DELETE answers 204 and the player is gone.
    let (status, _) = request(
        app.clone(),
        "DELETE",
        &format!("/v4/sessions/{session_id}/players/1234"),
        None,
    )
    .await;
    assert_eq!(status, 204);
    let (status, _) = request(
        app.clone(),
        "GET",
        &format!("/v4/sessions/{session_id}/players/1234"),
        None,
    )
    .await;
    assert_eq!(status, 404);
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/v4/routeplanner/status` — 204 when no planner is configured.
#[tokio::test]
async fn compat_v4_routeplanner_status_is_no_content() {
    let root = media_root();
    let response = app(&root)
        .oneshot(
            Request::builder()
                .uri("/v4/routeplanner/status")
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
    format!("ws://{address}/v4/websocket")
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

/// Row: `/v4/websocket` — the handshake rules, checked before the upgrade.
///
/// The reference refuses a missing, empty, unparseable or zero `User-Id`
/// with a 400, and refuses a wrong `Authorization` with a 401.
#[tokio::test]
async fn compat_v4_websocket_handshake_refuses_a_bad_request() {
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

/// Row: `/v4/websocket` — a wrong `Authorization` is a 401.
#[tokio::test]
async fn compat_v4_websocket_handshake_refuses_a_wrong_password() {
    let root = media_root();
    let sources = SourceRegistry::new();
    sources
        .register(Box::new(LocalMediaAdapter::new(&root).unwrap()))
        .unwrap();
    let address = serve(build_router(AppState {
        sessions: Arc::new(SessionManager::new()),
        v4_sessions: Arc::new(V4SessionManager::new()),
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

/// Row: `/v4/websocket` — the `ready` frame is flat and comes first.
#[tokio::test]
async fn compat_v4_websocket_ready_frame() {
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

/// Row: `/v4/info` — Lavalink v4 info shape.
#[tokio::test]
async fn compat_v4_info_shape() {
    let root = media_root();
    let value = get_json(app(&root), "/v4/info").await;

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

/// Row: `/v4/loadtracks` — track shape and a decodable encoded handle.
#[tokio::test]
async fn compat_v4_loadtracks_track_shape() {
    let root = media_root();
    let value = get_json(app(&root), "/v4/loadtracks?identifier=sample.wav").await;

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
    assert_eq!(V4Op::Ready.as_str(), "ready");
    assert_eq!(V4Op::Stats.as_str(), "stats");
    assert_eq!(V4Op::PlayerUpdate.as_str(), "playerUpdate");
    assert_eq!(V4Op::Event.as_str(), "event");
    assert_eq!(
        V4EventMessage::TrackStart {
            guild_id: "1".into(),
            track: track(),
        }
        .type_name(),
        "TrackStartEvent"
    );
    assert_eq!(
        V4EventMessage::TrackEnd {
            guild_id: "1".into(),
            track: track(),
            reason: nexora_protocol::V4TrackEndReason::Finished,
        }
        .type_name(),
        "TrackEndEvent"
    );
    assert_eq!(
        V4EventMessage::TrackException {
            guild_id: "1".into(),
            track: track(),
            exception: nexora_protocol::V4Exception {
                message: "boom".into(),
                severity: nexora_protocol::V4ErrorSeverity::Fault,
                cause: None,
                cause_stack_trace: None,
            },
        }
        .type_name(),
        "TrackExceptionEvent"
    );
    assert_eq!(
        V4EventMessage::WebSocketClosed {
            guild_id: "1".into(),
            code: 1000,
            reason: "bye".into(),
            by_remote: false,
        }
        .type_name(),
        "WebSocketClosedEvent"
    );
}
