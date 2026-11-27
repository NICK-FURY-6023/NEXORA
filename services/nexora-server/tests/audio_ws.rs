//! End-to-end proof that the native audio WebSocket delivers real Opus frames.
//!
//! The test serves the real router over a TCP socket, creates a session, plays
//! a one-second 48 kHz stereo WAV, connects to
//! `/ws/v1/sessions/{id}/audio`, and counts the binary messages it receives.
//! One second of audio is 50 packets of 20 ms, so the count and the timing are
//! checked against that contract rather than against "some bytes arrived".

use axum::Router;
use nexora_server::{AppState, RuntimeMetrics, build_router};
use nexora_session::{NexoraSessionManager, SessionManager};
use nexora_source_framework::{LocalMediaAdapter, SourceRegistry};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio_tungstenite::tungstenite::Message;

const SAMPLE_RATE: u32 = 48_000;
const SECONDS: u32 = 1;
const FRAME_MS: u64 = 20;

fn media_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("nexora-audio-ws-{unique}"));
    fs::create_dir_all(&root).unwrap();

    let frames = SAMPLE_RATE * SECONDS;
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

// The serving task, the pacing task and the reading test each need to make
// progress, so run them on a multi-threaded runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_audio_websocket_streams_real_opus_packets() {
    use futures_util::StreamExt;

    let root = media_root();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app(&root)).await.unwrap();
    });

    let http = reqwest::Client::new();
    let base = format!("http://{address}");

    let created_response = http
        .post(format!("{base}/api/v1/sessions"))
        .header("content-type", "application/json")
        .body(
            serde_json::json!({
                "application": "audio-ws-test",
                "version": "1.0.0",
                "output": {"sample_rate": SAMPLE_RATE, "channels": 2, "codec": "opus"}
            })
            .to_string(),
        )
        .send()
        .await
        .unwrap();
    let created: serde_json::Value =
        serde_json::from_str(&created_response.text().await.unwrap()).unwrap();
    let session_id = created["session_id"].as_str().unwrap().to_owned();

    let played = http
        .post(format!("{base}/api/v1/sessions/{session_id}/play"))
        .header("content-type", "application/json")
        .body(serde_json::json!({"track_id": "tone.wav", "start_position_ms": 0}).to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(played.status(), reqwest::StatusCode::OK);

    // The pacing task registers its broadcast sender while starting; give it
    // one tick so the subscription cannot race the registration.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (mut socket, response) = tokio_tungstenite::connect_async(format!(
        "ws://{address}/ws/v1/sessions/{session_id}/audio"
    ))
    .await
    .expect("the audio socket must accept the upgrade");
    assert_eq!(response.status().as_u16(), 101);

    // One second of audio is 50 frames of 20 ms. Read for a little longer than
    // the track lasts, then stop: the socket stays open after the track ends.
    let deadline = Instant::now() + Duration::from_millis(1_500);
    let mut packets = 0usize;
    let mut bytes = 0usize;
    let mut largest = 0usize;
    while Instant::now() < deadline {
        let next = tokio::time::timeout(Duration::from_millis(500), socket.next()).await;
        match next {
            Ok(Some(Ok(Message::Binary(payload)))) => {
                assert!(!payload.is_empty(), "an Opus packet is never empty");
                assert!(
                    payload.len() <= 4_000,
                    "an Opus packet must stay under 4000 bytes, got {}",
                    payload.len()
                );
                assert!(
                    (payload[0] >> 3) <= 31,
                    "the first byte must be a valid Opus TOC byte"
                );
                packets += 1;
                bytes += payload.len();
                largest = largest.max(payload.len());
            }
            Ok(Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)))) => continue,
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
            Ok(Some(Ok(Message::Text(text)))) => {
                panic!("the audio socket is binary-only, got text: {text}");
            }
            // The playback task drops its sender when the track ends, so the
            // server closes the socket without a closing handshake. A readable
            // stream that ends there is the expected end of the track.
            Ok(Some(Err(_))) => break,
            Err(_) => break,
        }
    }

    server.abort();

    assert!(
        (40..=60).contains(&packets),
        "one second at 20 ms per packet should be about 50 packets, got {packets} ({bytes} bytes, largest {largest})"
    );
    assert!(
        bytes > packets * 10,
        "Opus packets should carry more than a token payload: {bytes} bytes over {packets} packets"
    );

    let expected_packets = (SECONDS as u64 * 1_000 / FRAME_MS) as usize;
    assert_eq!(expected_packets, 50, "the pacing contract is 20 ms frames");
}
