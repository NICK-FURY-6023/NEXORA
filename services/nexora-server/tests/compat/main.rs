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
use nexora_protocol::V4EventType;
use nexora_protocol::V4Op;
use nexora_protocol::decode_track;
use nexora_server::{AppState, RuntimeMetrics, build_router};
use nexora_session::SessionManager;
use nexora_source_framework::{LocalMediaAdapter, SourceRegistry};
use tower::ServiceExt;

fn media_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("nexora-compat-{unique}"));
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
    assert!(value["plugins"].is_array());
    let _ = fs::remove_dir_all(&root);
}

/// Row: `/v4/loadtracks` — track shape and a decodable encoded handle.
#[tokio::test]
async fn compat_v4_loadtracks_track_shape() {
    let root = media_root();
    let value = get_json(app(&root), "/v4/loadtracks?identifier=sample.wav").await;

    assert_eq!(value["loadType"], "search");
    let track = &value["data"][0];
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
    assert_eq!(V4EventType::TrackStart.as_str(), "TrackStartEvent");
    assert_eq!(V4EventType::TrackEnd.as_str(), "TrackEndEvent");
    assert_eq!(V4EventType::TrackException.as_str(), "TrackExceptionEvent");
    assert_eq!(V4EventType::PlayerUpdate.as_str(), "playerUpdate");
    assert_eq!(
        V4EventType::WebSocketClosed.as_str(),
        "WebSocketClosedEvent"
    );
}
