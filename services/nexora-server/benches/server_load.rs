use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use criterion::{Criterion, criterion_group, criterion_main};
use nexora_server::{AppState, RuntimeMetrics, build_router};
use nexora_session::{NexoraSessionManager, SessionManager};
use nexora_source_framework::SourceRegistry;
use tower::ServiceExt;

fn app() -> axum::Router {
    build_router(AppState {
        sessions: Arc::new(SessionManager::new()),
        nexora_sessions: Arc::new(NexoraSessionManager::new()),
        sources: Arc::new(SourceRegistry::new()),
        password: None,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        voice_sockets: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        raw_streaming: false,
        plugins: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    })
}

fn benchmark_server_load(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime should start");

    c.bench_function("create_100_sessions_through_the_router", |bencher| {
        bencher.to_async(&runtime).iter(|| async {
            let app = app();
            for _ in 0..100 {
                let response = app
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
                assert_eq!(response.status(), StatusCode::CREATED);
            }
        });
    });
}

criterion_group!(benches, benchmark_server_load);
criterion_main!(benches);
