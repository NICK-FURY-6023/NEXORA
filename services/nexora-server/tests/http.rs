use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use nexora_server::{AppState, RuntimeMetrics, build_router};
use nexora_session::SessionManager;
use nexora_source_framework::{LocalMediaAdapter, SourceRegistry};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

fn test_state() -> AppState {
    AppState {
        sessions: Arc::new(SessionManager::new()),
        sources: Arc::new(SourceRegistry::new()),
        password: None,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    }
}

fn test_app() -> axum::Router {
    build_router(test_state())
}

fn temp_media_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("nexora-media-{unique}"));
    fs::create_dir_all(&root).unwrap();

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
    fs::write(root.join("tone.wav"), wav).unwrap();
    root
}

#[tokio::test]
async fn health_endpoint_is_available() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn nexora_session_create_and_play_contract() {
    let root = temp_media_root();
    let mut sources = SourceRegistry::new();
    sources
        .register(Box::new(LocalMediaAdapter::new(&root).unwrap()))
        .unwrap();
    let state = AppState {
        sessions: Arc::new(SessionManager::new()),
        sources: Arc::new(sources),
        password: None,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    };
    let app = build_router(state);
    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"application":"bot","version":"1.0.0","output":{"sample_rate":48000,"channels":2,"codec":"opus"}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(create.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let session_id = value["session_id"].as_str().unwrap();

    let play = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/play"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"track_id":"tone.wav","start_position_ms":0}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(play.status(), StatusCode::OK);
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn lavalink_compatible_info_endpoint() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/v4/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["name"], "NEXORA");
}

#[tokio::test]
async fn runtime_metrics_reflect_real_requests() {
    let app = test_app();
    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"application":"bot","version":"1.0.0","output":{"sample_rate":48000,"channels":2,"codec":"opus"}}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["nexora_active_sessions"].as_u64().unwrap() >= 1);
    assert!(value["nexora_api_requests_total"].as_u64().unwrap() >= 2);
}

#[tokio::test]
async fn lavalink_stats_uses_v4_cpu_field_names() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/v4/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["cpu"]["lavalinkLoad"].is_number());
}

#[tokio::test]
async fn lavalink_stats_endpoint() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/v4/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["op"], "stats");
}

#[tokio::test]
async fn lavalink_loadtracks_uses_v4_load_types() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/v4/loadtracks?identifier=definitely-not-a-track")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let load_type = value["loadType"].as_str().unwrap();
    assert!(
        matches!(
            load_type,
            "track" | "playlist" | "search" | "empty" | "error"
        ),
        "unexpected loadType: {load_type}"
    );
}

#[tokio::test]
async fn lavalink_loadtracks_returns_v4_track_shape() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("nexora-parity-{unique}"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("sample.wav"), b"not-a-real-wav").unwrap();

    let sources = SourceRegistry::new();
    sources
        .register(Box::new(LocalMediaAdapter::new(&root).unwrap()))
        .unwrap();
    let state = AppState {
        sessions: Arc::new(SessionManager::new()),
        sources: Arc::new(sources),
        password: None,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    };
    let response = build_router(state)
        .oneshot(
            Request::builder()
                .uri("/v4/loadtracks?identifier=sample.wav")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(value["loadType"], "search");
    let track = &value["data"][0];
    let encoded = track["encoded"].as_str().expect("encoded track");
    assert!(
        !base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap()
            .is_empty()
    );
    assert_eq!(track["info"]["title"], "sample.wav");
    assert_eq!(track["info"]["sourceName"], "local-media");
    assert_eq!(track["info"]["isSeekable"], false);
    assert_eq!(track["info"]["length"], 0);
    assert_eq!(track["info"]["isStream"], false);
    assert_eq!(track["pluginInfo"], serde_json::json!({}));

    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn lavalink_info_matches_documented_v4_shape() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/v4/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(value["version"]["semver"].is_string());
    assert!(value["version"]["major"].is_u64());
    assert!(value["version"]["minor"].is_u64());
    assert!(value["version"]["patch"].is_u64());
    assert!(value["buildTime"].is_u64());
    assert!(value["git"]["branch"].is_string());
    assert!(value["git"]["commit"].is_string());
    assert!(value["git"]["commitTime"].is_u64());
    assert!(value["jvm"].is_string());
    assert!(value["lavaplayer"].is_string());
    assert!(value["sourceManagers"].is_array());
    assert!(value["filters"].is_array());
    assert!(value["plugins"].is_array());
}

#[tokio::test]
async fn lavalink_player_matches_documented_v4_shape() {
    let app = test_app();
    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v4/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(created.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let session_id = created["sessionId"].as_str().unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v4/sessions/{session_id}/player"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(value["guildId"], session_id);
    assert!(value["volume"].is_u64());
    assert!(value["paused"].is_boolean());
    assert!(value["state"]["time"].is_u64());
    assert!(value["state"]["position"].is_u64());
    assert!(value["state"]["connected"].is_boolean());
    assert!(value["state"]["ping"].is_i64());
    assert!(value["filters"].is_object());
}
