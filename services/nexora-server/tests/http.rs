use axum::body::Body;
use axum::http::{Request, StatusCode};
use nexora_server::{AppState, build_router};
use nexora_session::SessionManager;
use nexora_source_framework::SourceRegistry;
use std::sync::Arc;
use tower::ServiceExt;

fn test_app() -> axum::Router {
    let state = AppState {
        sessions: Arc::new(SessionManager::new()),
        sources: Arc::new(SourceRegistry::new()),
        password: None,
    };
    build_router(state)
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
    let app = test_app();
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
                    r#"{"track_id":"track_1","start_position_ms":0}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(play.status(), StatusCode::OK);
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
