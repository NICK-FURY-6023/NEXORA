//! End-to-end proof of the Lavalink-compatible event stream.
//!
//! The test serves the real router on a TCP port, opens `/nexora/websocket`
//! with a real WebSocket client, starts a one-second local track through the
//! player route, and reads the frames until the track ends on its own. It
//! asserts the reference's frame shape (`op`, `type` and the event fields are
//! siblings, with no `data` wrapper) and the reference's order: the `ready`
//! frame first, then the player-created event, the track-start event, an
//! immediate `playerUpdate`, and finally a `TrackEndEvent` whose reason is
//! `finished` — a natural end, not a stop the test triggered.

use axum::Router;
use nexora_server::{AppState, RuntimeMetrics, build_router};
use nexora_session::{NexoraSessionManager, SessionManager};
use nexora_source_framework::{LocalMediaAdapter, SourceRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const SAMPLE_RATE: u32 = 48_000;

fn media_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("nexora-event-ws-{unique}"));
    fs::create_dir_all(&root).unwrap();

    let frames = SAMPLE_RATE; // one second
    let data_len = frames * 2 * 2;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 4).to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for index in 0..frames {
        let value =
            ((index as f32 / SAMPLE_RATE as f32) * std::f32::consts::TAU * 440.0).sin() * 0.4;
        let sample = (value * 32_767.0) as i16;
        wav.extend_from_slice(&sample.to_le_bytes());
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(root.join("tone.wav"), wav).unwrap();
    root
}

fn app(root: &PathBuf) -> Router {
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
        voice_sockets: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        raw_streaming: false,
        plugins: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    })
}

async fn send_json(
    http: &reqwest::Client,
    method: reqwest::Method,
    url: String,
    body: Option<Value>,
) -> reqwest::Response {
    let mut request = http.request(method, url);
    if let Some(body) = body {
        request = request
            .header("content-type", "application/json")
            .body(body.to_string());
    }
    request.send().await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_event_websocket_reports_a_whole_track() {
    use futures_util::StreamExt;

    let root = media_root();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app(&root)).await.unwrap();
    });

    let http = reqwest::Client::new();
    let base = format!("http://{address}");

    // The reference's route for a session; the socket itself is opened with a
    // `User-Id`, which the handshake requires to be a non-zero integer.
    let created = send_json(
        &http,
        reqwest::Method::POST,
        format!("{base}/nexora/sessions"),
        None,
    )
    .await;
    assert_eq!(created.status(), reqwest::StatusCode::CREATED);
    let session_id =
        serde_json::from_str::<Value>(&created.text().await.unwrap()).unwrap()["sessionId"]
            .as_str()
            .unwrap()
            .to_owned();

    let loaded = send_json(
        &http,
        reqwest::Method::GET,
        format!("{base}/nexora/loadtracks?identifier=tone.wav"),
        None,
    )
    .await;
    let loaded: Value = serde_json::from_str(&loaded.text().await.unwrap()).unwrap();
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("the local track has an encoded form")
        .to_owned();

    let mut request = format!("ws://{address}/nexora/websocket")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("User-Id", "1234".parse().unwrap());
    let (mut socket, response) = tokio_tungstenite::connect_async(request)
        .await
        .expect("the event socket upgrades");
    assert_eq!(response.status().as_u16(), 101);

    // Starts playback once the socket is listening, so no frame is missed.
    let patched = send_json(
        &http,
        reqwest::Method::PATCH,
        format!("{base}/nexora/sessions/{session_id}/players/1234"),
        Some(serde_json::json!({"track": {"encoded": encoded}})),
    )
    .await;
    assert_eq!(patched.status(), reqwest::StatusCode::OK);

    let deadline = Instant::now() + Duration::from_secs(6);
    let mut frames: Vec<Value> = Vec::new();
    while Instant::now() < deadline {
        let next = tokio::time::timeout(Duration::from_secs(3), socket.next()).await;
        let frame = match next {
            Ok(Some(Ok(Message::Text(text)))) => {
                serde_json::from_str::<Value>(&text).expect("every frame is JSON")
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(error))) => panic!("the event socket failed: {error}"),
            Err(_) => break,
        };
        let is_end = frame["type"] == "TrackEndEvent";
        frames.push(frame);
        if is_end {
            break;
        }
    }
    server.abort();

    assert!(!frames.is_empty(), "the socket sends frames");

    // Every frame is flat: `op` is present, `data` never wraps the payload.
    for frame in &frames {
        assert!(
            frame["op"].is_string(),
            "every frame carries an op: {frame}"
        );
        assert!(
            frame.get("data").is_none(),
            "a frame has no data wrapper: {frame}"
        );
    }

    // The ready frame comes first and is not a resume.
    let ready = &frames[0];
    assert_eq!(ready["op"], "ready");
    assert_eq!(ready["resumed"], false);
    assert!(ready["sessionId"].is_string());

    // Then the player is created and the track starts; event frames carry the
    // discriminator, the guild id and their own fields as siblings.
    let created = frames
        .iter()
        .find(|frame| frame["type"] == "PlayerCreatedEvent")
        .expect("the player-created event is reported");
    assert_eq!(created["guildId"], "1234");

    let start = frames
        .iter()
        .find(|frame| frame["type"] == "TrackStartEvent")
        .expect("the track-start event is reported");
    assert_eq!(start["op"], "event");
    assert_eq!(start["guildId"], "1234");
    assert_eq!(start["track"]["info"]["identifier"], "tone.wav");
    assert_eq!(start["track"]["info"]["sourceName"], "local-media");
    assert!(start["track"]["encoded"].is_string());

    // The reference's order for one track: start, then an immediate update.
    let start_index = frames
        .iter()
        .position(|frame| frame["type"] == "TrackStartEvent")
        .unwrap();
    let update = frames[start_index..]
        .iter()
        .find(|frame| frame["op"] == "playerUpdate")
        .expect("a playerUpdate follows the start event");
    assert_eq!(update["guildId"], "1234");
    assert!(update["state"]["time"].is_u64());
    assert!(update["state"]["position"].is_u64());
    assert_eq!(update["state"]["connected"], false);
    assert_eq!(update["state"]["ping"], -1);

    // The track ran to its natural end, so the reason is `finished`, and the
    // end arrives after the start rather than being triggered by the test.
    let end = frames
        .iter()
        .find(|frame| frame["type"] == "TrackEndEvent")
        .expect("the track-end event is reported");
    assert_eq!(end["op"], "event");
    assert_eq!(end["guildId"], "1234");
    assert_eq!(end["reason"], "finished");
    assert_eq!(end["track"]["info"]["identifier"], "tone.wav");

    let end_index = frames
        .iter()
        .position(|frame| frame["type"] == "TrackEndEvent")
        .unwrap();
    assert!(
        end_index > start_index,
        "the end follows the start (start {start_index}, end {end_index})"
    );
}
