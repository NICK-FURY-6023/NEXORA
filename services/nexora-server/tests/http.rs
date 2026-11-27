use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use nexora_common::SessionId;
use nexora_protocol::SessionEventType;
use nexora_server::{AppState, RuntimeMetrics, build_router};
use nexora_session::{NexoraSessionManager, SessionManager};
use nexora_source_framework::{LocalMediaAdapter, SourceRegistry};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

fn test_state() -> AppState {
    AppState {
        sessions: Arc::new(SessionManager::new()),
        nexora_sessions: Arc::new(NexoraSessionManager::new()),
        sources: Arc::new(SourceRegistry::new()),
        password: None,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        voice_sockets: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        raw_streaming: false,
        plugins: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    }
}

fn test_app() -> axum::Router {
    build_router(test_state())
}

fn temp_media_root() -> PathBuf {
    // The clock's resolution is coarse on Windows, so two tests can pick the
    // same timestamp; the counter keeps every root distinct.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("nexora-media-{unique}-{sequence}"));
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
async fn every_builtin_source_adapter_is_registered() {
    use nexora_server::register_builtin_sources;
    use nexora_source_universal::oauth::TokenStore;

    let sources = SourceRegistry::new();
    register_builtin_sources(&sources, Arc::new(TokenStore::new()));

    let ids = sources.source_ids();
    for expected in [
        "internet-archive",
        "bandcamp-public",
        "soundcloud-public",
        "youtube",
        "spotify",
        "deezer",
        "apple-music",
        "tidal",
    ] {
        assert!(
            ids.contains(&expected),
            "the {expected} adapter is registered: {ids:?}"
        );
    }
}

#[tokio::test]
async fn fetching_an_http_source_refuses_a_private_target() {
    use nexora_server::download_source_to_temp;

    // The download path runs the source framework's SSRF validation before it
    // fetches anything, so a private-network target never reaches the network.
    for url in [
        "http://127.0.0.1/stream",
        "http://localhost/stream",
        "http://10.0.0.1/stream",
        "http://192.168.1.1/stream",
        "http://[::1]/stream",
        "http://2130706433/stream",
    ] {
        assert!(
            download_source_to_temp(url).await.is_err(),
            "{url} must be refused"
        );
    }
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
    let sources = SourceRegistry::new();
    sources
        .register(Box::new(LocalMediaAdapter::new(&root).unwrap()))
        .unwrap();
    let state = AppState {
        sessions: Arc::new(SessionManager::new()),
        nexora_sessions: Arc::new(NexoraSessionManager::new()),
        sources: Arc::new(sources),
        password: None,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        voice_sockets: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        raw_streaming: false,
        plugins: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
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
async fn playback_advances_to_the_next_queued_track() {
    let root = temp_media_root();
    fs::copy(root.join("tone.wav"), root.join("tone2.wav")).unwrap();
    let sources = SourceRegistry::new();
    sources
        .register(Box::new(LocalMediaAdapter::new(&root).unwrap()))
        .unwrap();
    let sessions = Arc::new(SessionManager::new());
    let mut events = sessions.subscribe();
    let state = AppState {
        sessions: Arc::clone(&sessions),
        nexora_sessions: Arc::new(NexoraSessionManager::new()),
        sources: Arc::new(sources),
        password: None,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        voice_sockets: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        raw_streaming: false,
        plugins: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
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

    let enqueue = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/queue"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"track_id":"tone2.wav"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enqueue.status(), StatusCode::OK);

    let play = app
        .clone()
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

    let mut ended = false;
    let mut started_next = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        let Ok(Ok(event)) = tokio::time::timeout(Duration::from_secs(2), events.recv()).await
        else {
            continue;
        };
        match event.event_type {
            SessionEventType::TrackEnded => ended = true,
            SessionEventType::TrackStarted if ended && event.payload["track_id"] == "tone2.wav" => {
                started_next = true;
                break;
            }
            _ => {}
        }
    }
    assert!(ended, "expected TrackEnded for the finished track");
    assert!(started_next, "expected the queued track to start");

    let session = SessionId::new(session_id.to_string()).unwrap();
    assert_eq!(
        sessions
            .current_track(&session)
            .await
            .unwrap()
            .map(|track| track.as_str().to_string()),
        Some("tone2.wav".to_string())
    );
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn nexora_compatible_info_endpoint() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/nexora/info")
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
async fn nexora_stats_uses_nexora_cpu_field_names() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/nexora/stats")
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
async fn nexora_stats_endpoint() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/nexora/stats")
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
    // The REST payload is a `StatsData`: only the WebSocket `stats` frame
    // carries an `op`.
    assert!(value.get("op").is_none());
    assert!(value["players"].is_u64());
    assert!(value["playingPlayers"].is_u64());
    assert!(value["uptime"].is_u64());
    assert!(value["memory"].is_object());
    assert!(value["cpu"].is_object());
}

#[tokio::test]
async fn nexora_loadtracks_uses_nexora_load_types() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/nexora/loadtracks?identifier=definitely-not-a-track")
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
async fn nexora_loadtracks_returns_nexora_track_shape() {
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
        nexora_sessions: Arc::new(NexoraSessionManager::new()),
        sources: Arc::new(sources),
        password: None,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        voice_sockets: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        raw_streaming: false,
        plugins: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    };
    let response = build_router(state)
        .oneshot(
            Request::builder()
                .uri("/nexora/loadtracks?identifier=sample.wav")
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
    // The reference's `SEARCH_RESULT` payload is `{ "tracks": [...] }`.
    let track = &value["data"]["tracks"][0];
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
async fn nexora_info_matches_documented_nexora_shape() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/nexora/info")
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
    // The reference nests the plugin list under a `plugins` object.
    assert!(value["plugins"]["plugins"].is_array());
}

#[tokio::test]
async fn nexora_info_reports_the_typed_nexora_shape() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/nexora/info")
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
    assert!(value["version"]["major"].is_number());
    assert!(value["buildTime"].is_number());
    assert!(value["git"]["branch"].is_string());
    assert!(value["filters"].is_array());
    assert!(value["sourceManagers"].is_array());
}

#[tokio::test]
async fn nexora_player_matches_documented_nexora_shape() {
    let app = test_app();
    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/nexora/sessions")
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

    // A missing player is a 404 in the reference, so create it with a PATCH
    // first; the player route is `/nexora/sessions/{sessionId}/players/{guildId}`.
    let created_player = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/nexora/sessions/{session_id}/players/1234"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"paused":true,"volume":80}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created_player.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/nexora/sessions/{session_id}/players/1234"))
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

    assert_eq!(value["guildId"], "1234");
    assert_eq!(value["volume"], 80);
    assert_eq!(value["paused"], true);
    assert!(value["volume"].is_u64());
    assert!(value["paused"].is_boolean());
    assert!(value["state"]["time"].is_u64());
    assert!(value["state"]["position"].is_u64());
    assert!(value["state"]["connected"].is_boolean());
    assert!(value["state"]["ping"].is_i64());
    assert!(value["filters"].is_object());
}
