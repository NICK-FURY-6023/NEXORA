use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use nexora_audio_pipeline::{OutputCodec, process_file};
use nexora_audio_types::AudioFormat;
use nexora_common::{SessionId, TrackId};
use nexora_error::NexoraError;
use nexora_protocol::{CreateSessionRequest, HealthStatus, PlayRequest, SearchRequest};
use nexora_session::SessionManager;
use nexora_source_api::{AuthContext, ResolvedSource, SourceError, TrackReference};
use nexora_source_framework::{SourceRegistry, rank_candidates};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

#[derive(Debug, Default)]
pub struct RuntimeMetrics {
    active_sessions: AtomicU64,
    api_requests: AtomicU64,
    search_requests: AtomicU64,
    stream_bytes: AtomicU64,
    errors: AtomicU64,
}

impl RuntimeMetrics {
    pub fn active_sessions(&self) -> u64 {
        self.active_sessions.load(Ordering::Relaxed)
    }

    pub fn session_created(&self) {
        self.active_sessions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn session_stopped(&self) {
        let _ = self
            .active_sessions
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_sub(1))
            });
    }

    pub fn request(&self) {
        self.api_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn search(&self) {
        self.search_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn streamed(&self, bytes: usize) {
        self.stream_bytes.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub fn error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<SessionManager>,
    pub sources: Arc<SourceRegistry>,
    pub password: Option<String>,
    pub metrics: Arc<RuntimeMetrics>,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

impl ApiError {
    fn bad_request() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_INPUT",
            message: "invalid request",
        }
    }

    fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND",
            message: "resource not found",
        }
    }

    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            message: "authorization required",
        }
    }

    fn unsupported() -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "UNSUPPORTED_SOURCE",
            message: "source type cannot be streamed yet",
        }
    }
}

impl From<SourceError> for ApiError {
    fn from(error: SourceError) -> Self {
        match error {
            SourceError::Unauthorized => Self::unauthorized(),
            SourceError::NotFound => Self::not_found(),
            SourceError::InvalidInput => Self::bad_request(),
            SourceError::Unavailable | SourceError::Timeout | SourceError::RateLimited => Self {
                status: StatusCode::SERVICE_UNAVAILABLE,
                code: "SOURCE_UNAVAILABLE",
                message: "source temporarily unavailable",
            },
            _ => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "INTERNAL",
                message: "internal server error",
            },
        }
    }
}

impl From<NexoraError> for ApiError {
    fn from(error: NexoraError) -> Self {
        match error {
            NexoraError::InvalidInput { .. } | NexoraError::Config { .. } => Self::bad_request(),
            NexoraError::Service { .. } => Self::not_found(),
            _ => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "INTERNAL",
                message: "internal server error",
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({ "error": { "code": self.code, "message": self.message } })),
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize)]
pub struct SeekRequest {
    pub position_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct VolumeRequest {
    pub volume: u8,
}

#[derive(Debug, Deserialize)]
pub struct QueueRequest {
    pub track_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub identifier: String,
}

fn authorize(headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    state.metrics.request();
    let Some(expected) = state.password.as_deref() else {
        return Ok(());
    };
    let supplied = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if supplied == expected {
        Ok(())
    } else {
        state.metrics.error();
        Err(ApiError::unauthorized())
    }
}

fn parse_session_id(value: &str) -> Result<SessionId, ApiError> {
    SessionId::new(value).map_err(|_| ApiError::bad_request())
}

fn internal_auth() -> AuthContext {
    AuthContext::new(
        "nexora-server",
        vec![
            "media.local".to_string(),
            "radio.public".to_string(),
            "http.stream".to_string(),
        ],
    )
    .expect("static auth context")
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/info", get(info))
        .route("/api/v1/sources", get(sources))
        .route("/api/v1/search", post(search))
        .route("/api/v1/sessions", post(create_session))
        .route("/api/v1/sessions/{id}", get(get_session))
        .route("/api/v1/sessions/{id}/play", post(play))
        .route("/api/v1/sessions/{id}/pause", post(pause))
        .route("/api/v1/sessions/{id}/resume", post(resume))
        .route("/api/v1/sessions/{id}/stop", post(stop))
        .route("/api/v1/sessions/{id}/seek", post(seek))
        .route("/api/v1/sessions/{id}/volume", post(set_volume))
        .route("/api/v1/sessions/{id}/queue", get(get_queue).post(enqueue))
        .route("/api/v1/sessions/{id}/stream", get(stream_audio))
        .route("/api/v1/metrics", get(metrics))
        .route("/ws/v1/sessions/{id}", get(websocket_events))
        .route("/v4/info", get(lavalink_info))
        .route("/v4/stats", get(lavalink_stats))
        .route("/v4/routeplanner/status", get(lavalink_routeplanner_status))
        .route("/v4/loadtracks", get(lavalink_loadtracks))
        .route("/v4/sessions", post(lavalink_create_session))
        .route(
            "/v4/sessions/{id}/player",
            get(lavalink_player).patch(lavalink_patch_player),
        )
        .route("/v4/sessions/{id}", get(websocket_events))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthStatus> {
    Json(HealthStatus {
        service: nexora_common::ServiceName::Gateway,
        healthy: true,
        message: format!("sessions={}", state.metrics.active_sessions()),
    })
}

async fn info() -> Json<Value> {
    Json(json!({
        "engine": "nexora",
        "api_version": "v1",
        "lavalink_compatible": true,
        "features": ["rest", "websocket", "sessions", "source-resolution", "opus-output"]
    }))
}

async fn sources(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let ids: Vec<&'static str> = state
        .sources
        .entries()
        .into_iter()
        .map(|entry| entry.source_id)
        .collect();
    Ok(Json(json!({ "sources": ids })))
}

async fn search(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    state.metrics.search();
    let mut results = state
        .sources
        .search(&request.query, &internal_auth())
        .await?;
    results = rank_candidates(
        results,
        &request.query,
        request
            .sources
            .as_deref()
            .and_then(|values| values.first())
            .map(String::as_str),
    );
    let limit = request.limit.unwrap_or(10).min(50);
    results.truncate(limit);
    Ok(Json(json!({ "results": results })))
}

async fn create_session(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    authorize(&headers, &state)?;
    let response = state.sessions.create_from_request(request).await?;
    state.metrics.session_created();
    Ok((StatusCode::CREATED, Json(json!(response))))
}

async fn get_session(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    Ok(Json(json!(state.sessions.get(&session_id).await?)))
}

async fn play(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<PlayRequest>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    state.sessions.play(&session_id, request).await?;
    Ok(Json(json!({ "status": "playing" })))
}

async fn pause(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    state.sessions.pause(&session_id).await?;
    Ok(Json(json!({ "status": "paused" })))
}

async fn resume(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    state.sessions.resume(&session_id).await?;
    Ok(Json(json!({ "status": "playing" })))
}

async fn stop(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    state.sessions.stop(&session_id).await?;
    state.metrics.session_stopped();
    Ok(Json(json!({ "status": "stopped" })))
}

async fn seek(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<SeekRequest>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    state
        .sessions
        .seek(&session_id, request.position_ms)
        .await?;
    Ok(Json(json!({ "position_ms": request.position_ms })))
}

async fn set_volume(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<VolumeRequest>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    state
        .sessions
        .set_volume(&session_id, request.volume)
        .await?;
    Ok(Json(json!({ "volume": request.volume })))
}

async fn get_queue(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    let queue = state.sessions.queue(&session_id).await?;
    Ok(Json(json!({ "queue": queue })))
}

async fn enqueue(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<QueueRequest>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    let track_id = TrackId::new(request.track_id).map_err(|_| ApiError::bad_request())?;
    let len = state.sessions.enqueue(&session_id, track_id).await?;
    Ok(Json(json!({ "queue_length": len })))
}

async fn stream_audio(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    let track_id = state
        .sessions
        .current_track(&session_id)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let format: AudioFormat = state.sessions.output_format(&session_id).await?;
    let resolved = state
        .sources
        .resolve(&TrackReference::new(track_id.as_str()), &internal_auth())
        .await?;
    let ResolvedSource::LocalFile { path } = resolved else {
        return Err(ApiError::unsupported());
    };

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let output = std::env::temp_dir().join(format!("nexora-stream-{unique}.opus"));
    let input = path.clone();
    let output_path = output.clone();
    tokio::task::spawn_blocking(move || {
        process_file(
            &input,
            &output_path,
            format.sample_rate,
            format.channels,
            OutputCodec::Opus,
        )
    })
    .await
    .map_err(|_| ApiError::not_found())??;

    let bytes = tokio::fs::read(&output)
        .await
        .map_err(|_| ApiError::not_found())?;
    state.metrics.streamed(bytes.len());
    let _ = tokio::fs::remove_file(&output).await;
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "audio/ogg")],
        Body::from(bytes),
    )
        .into_response())
}

async fn metrics(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    Ok(Json(json!({
        "nexora_active_sessions": state.metrics.active_sessions(),
        "nexora_api_requests_total": state.metrics.api_requests.load(Ordering::Relaxed),
        "nexora_search_requests_total": state.metrics.search_requests.load(Ordering::Relaxed),
        "nexora_stream_bytes_total": state.metrics.stream_bytes.load(Ordering::Relaxed),
        "nexora_api_errors_total": state.metrics.errors.load(Ordering::Relaxed)
    })))
}

async fn websocket_events(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if let Err(error) = authorize(&headers, &state) {
        return error.into_response();
    }
    let session_id = match parse_session_id(&id) {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    upgrade.on_upgrade(move |socket| forward_events(socket, state.sessions.subscribe(), session_id))
}

async fn forward_events(
    mut socket: WebSocket,
    mut receiver: tokio::sync::broadcast::Receiver<nexora_protocol::EventEnvelope>,
    session_id: SessionId,
) {
    loop {
        match receiver.recv().await {
            Ok(event) => {
                if event.session_id != session_id {
                    continue;
                }
                let Ok(text) = serde_json::to_string(&event) else {
                    continue;
                };
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => break,
        }
    }
}

async fn lavalink_stats(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    Ok(Json(json!({
        "op": "stats",
        "players": state.metrics.active_sessions(),
        "playingPlayers": state.metrics.active_sessions(),
        "uptime": 0,
        "memory": { "free": 0, "used": 0, "allocated": 0, "reservable": 0 },
        "cpu": { "cores": 0, "systemLoad": 0.0, "processLoad": 0.0 },
        "frameStats": null
    })))
}

async fn lavalink_routeplanner_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    Ok(Json(json!({ "class": null, "details": {} })))
}

async fn lavalink_info() -> Json<Value> {
    Json(json!({
        "name": "NEXORA",
        "version": "0.1.0",
        "buildTime": "2026-11-22T00:00:00Z",
        "git": { "branch": "main", "commit": "nexora" },
        "jvm": "rust-native",
        "lavaplayer": "nexora-audio-engine"
    }))
}

async fn lavalink_loadtracks(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let mut results = state
        .sources
        .search(&query.identifier, &internal_auth())
        .await?;
    results = rank_candidates(results, &query.identifier, None);
    results.truncate(25);
    Ok(Json(
        json!({ "loadType": "SEARCH_RESULT", "data": results }),
    ))
}

async fn lavalink_create_session(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let response = state
        .sessions
        .create(AudioFormat::new(
            48_000,
            2,
            nexora_audio_types::AudioCodec::Opus,
        ))
        .await?;
    Ok(Json(json!({ "sessionId": response.session_id.as_str() })))
}

async fn lavalink_player(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    let state = state.sessions.get(&session_id).await?;
    Ok(Json(json!({
        "guildId": session_id.as_str(),
        "track": null,
        "volume": state.volume,
        "paused": state.status == "paused",
        "state": { "status": state.status, "position": state.position_ms }
    })))
}

#[derive(Debug, Deserialize)]
struct LavalinkPlayerPatch {
    paused: Option<bool>,
    volume: Option<u8>,
    identifier: Option<String>,
}

async fn lavalink_patch_player(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(patch): Json<LavalinkPlayerPatch>,
) -> Result<Json<Value>, ApiError> {
    authorize(&headers, &state)?;
    let session_id = parse_session_id(&id)?;
    if let Some(paused) = patch.paused {
        if paused {
            state.sessions.pause(&session_id).await?;
        } else {
            state.sessions.resume(&session_id).await?;
        }
    }
    if let Some(volume) = patch.volume {
        state.sessions.set_volume(&session_id, volume).await?;
    }
    if let Some(identifier) = patch.identifier {
        let track_id = TrackId::new(identifier).map_err(|_| ApiError::bad_request())?;
        state
            .sessions
            .play(
                &session_id,
                PlayRequest {
                    track_id,
                    start_position_ms: 0,
                },
            )
            .await?;
    }
    lavalink_player(headers, State(state), Path(id)).await
}
