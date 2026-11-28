use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use nexora_audio_pipeline::filters::xsecret_key;
use nexora_audio_pipeline::voice::{VoiceStream, derive_ssrc, parse_voice_endpoint};
use nexora_audio_pipeline::{OpusFrame, OutputCodec, encode_file_packets, process_file};
use nexora_audio_types::AudioFormat;
use nexora_common::{SessionId, TrackId};
use nexora_error::{NexoraError, NexoraResult};
use nexora_http::{DownloadedMedia, HttpClientConfig, NexoraHttpClient};
use nexora_protocol::{
    CreateSessionRequest, HealthStatus, NexoraErrorSeverity, NexoraEventMessage, NexoraException,
    NexoraFilters, NexoraPlayerState, NexoraPlayerUpdateFrame, NexoraTrackEndReason, PlayRequest,
    SearchRequest,
};
use nexora_session::{
    NexoraPlayerRecord, NexoraSessionManager, PLAYER_UPDATE_INTERVAL_SECONDS, PlaybackSignal,
    SessionManager,
};
use nexora_source_api::{AuthContext, ResolvedSource, SourceError, TrackReference};
use nexora_source_framework::{SourceRegistry, rank_candidates};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, watch};

pub mod nexora;

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
    pub nexora_sessions: Arc<NexoraSessionManager>,
    pub sources: Arc<SourceRegistry>,
    pub password: Option<String>,
    pub metrics: Arc<RuntimeMetrics>,
    pub audio: AudioChannels,
    /// The UDP voice sockets, one per player.
    ///
    /// The reference holds one media connection per player, so a track change
    /// keeps the socket and only the player's destruction closes it.
    pub voice_sockets: VoiceSockets,
    /// Whether the raw PCM streaming endpoint is served.
    pub raw_streaming: bool,
    /// The plugins the server loaded.
    pub plugins: PluginRegistry,
}

/// The UDP voice sockets, keyed by session id and guild id.
pub type VoiceSockets =
    Arc<tokio::sync::RwLock<HashMap<(String, String), Arc<tokio::net::UdpSocket>>>>;

/// One plugin the server can run.
#[derive(Clone)]
pub struct LoadedPlugin {
    /// The plugin's manifest, as it declared it.
    pub manifest: nexora_plugin_api::PluginManifest,
    /// The module's bytes.
    pub wasm: Arc<Vec<u8>>,
}

/// The plugins the server loaded, keyed by name.
pub type PluginRegistry = Arc<tokio::sync::RwLock<HashMap<String, LoadedPlugin>>>;

/// Loads the plugins in a directory.
///
/// A plugin is a `<name>.wasm` module with a `<name>.json` manifest beside
/// it. A module without a manifest, or with a manifest that does not
/// validate, is skipped: one broken plugin must not stop the server.
pub async fn load_plugins(directory: &std::path::Path) -> Vec<LoadedPlugin> {
    let Ok(entries) = tokio::fs::read_dir(directory).await else {
        return Vec::new();
    };
    let mut plugins = Vec::new();
    let mut modules: Vec<(String, std::path::PathBuf)> = Vec::new();
    let mut entries = entries;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("wasm") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        modules.push((stem.to_owned(), path));
    }
    modules.sort();
    for (stem, module_path) in modules {
        let manifest_path = module_path.with_extension("json");
        let Ok(manifest_bytes) = tokio::fs::read(&manifest_path).await else {
            tracing::warn!(%stem, "the plugin has no manifest and is skipped");
            continue;
        };
        let manifest: nexora_plugin_api::PluginManifest =
            match serde_json::from_slice(&manifest_bytes) {
                Ok(manifest) => manifest,
                Err(error) => {
                    tracing::warn!(%stem, %error, "the plugin manifest cannot be read");
                    continue;
                }
            };
        if let Err(error) = nexora_plugin_api::validate_manifest(&manifest) {
            tracing::warn!(%stem, %error, "the plugin manifest is not valid");
            continue;
        }
        let Ok(wasm) = tokio::fs::read(&module_path).await else {
            tracing::warn!(%stem, "the plugin module cannot be read");
            continue;
        };
        plugins.push(LoadedPlugin {
            manifest,
            wasm: Arc::new(wasm),
        });
    }
    plugins
}

/// Returns the player's voice socket, binding one the first time.
///
/// A bind failure leaves the player without a socket: the audio still plays,
/// it just cannot reach the voice gateway.
async fn voice_socket_for(
    state: &AppState,
    session_id: &str,
    guild_id: &str,
) -> Option<Arc<tokio::net::UdpSocket>> {
    let key = (session_id.to_owned(), guild_id.to_owned());
    if let Some(socket) = state.voice_sockets.read().await.get(&key) {
        return Some(Arc::clone(socket));
    }
    let socket = Arc::new(tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?);
    let mut sockets = state.voice_sockets.write().await;
    // Another task may have bound one while this one waited for the lock.
    if let Some(existing) = sockets.get(&key) {
        return Some(Arc::clone(existing));
    }
    sockets.insert(key, Arc::clone(&socket));
    Some(socket)
}

/// Closes the player's voice socket.
///
/// The socket is dropped once the last holder lets go of it, which is what
/// closes the port.
pub async fn close_voice_socket(state: &AppState, session_id: &str, guild_id: &str) {
    state
        .voice_sockets
        .write()
        .await
        .remove(&(session_id.to_owned(), guild_id.to_owned()));
}

pub type AudioChannels = Arc<tokio::sync::RwLock<HashMap<SessionId, broadcast::Sender<OpusFrame>>>>;

/// Downloads an HTTP source into a temporary file the decoder can read.
///
/// The request goes through the shared HTTP client, which refuses
/// private-network targets, caps the response size, and streams the body into
/// a temporary file. The returned owner keeps that file alive and removes it
/// when it is dropped, so a failed download, a cancelled task and a finished
/// playback all clean up after themselves.
pub async fn download_source_to_temp(url: &str) -> NexoraResult<DownloadedMedia> {
    shared_http_client()
        .download_to_temp(url)
        .await
        .map_err(|error| {
            tracing::warn!(%url, %error, "the source could not be downloaded");
            error
        })
}

/// The process-wide HTTP client.
///
/// One client means one connection pool and one egress policy for the whole
/// process.
pub fn shared_http_client() -> &'static NexoraHttpClient {
    static CLIENT: OnceLock<NexoraHttpClient> = OnceLock::new();
    CLIENT.get_or_init(|| {
        NexoraHttpClient::new(HttpClientConfig::default())
            .expect("the default http configuration is valid")
    })
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>, path: impl Into<String>) -> Self {
        let _ = path;
        Self {
            status,
            code: "REQUEST_FAILED",
            message: message.into(),
        }
    }

    fn bad_request() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_INPUT",
            message: "invalid request".to_owned(),
        }
    }

    fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND",
            message: "resource not found".to_owned(),
        }
    }

    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            message: "authorization required".to_owned(),
        }
    }

    fn unsupported() -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "UNSUPPORTED_SOURCE",
            message: "source type cannot be streamed yet".to_owned(),
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
                message: "source temporarily unavailable".to_owned(),
            },
            _ => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: "INTERNAL",
                message: "internal server error".to_owned(),
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
                message: "internal server error".to_owned(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthFailure {
    Missing,
    Invalid,
}

/// Constant-time equality over byte slices of possibly different lengths.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = 0u8;
    for index in 0..left.len().max(right.len()) {
        difference |=
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0);
    }
    difference == 0
}

/// The single credential check used by every REST and WebSocket route.
pub(crate) fn check_credential(
    headers: &HeaderMap,
    state: &AppState,
    count_request: bool,
) -> Result<(), AuthFailure> {
    if count_request {
        state.metrics.request();
    }
    let Some(expected) = state.password.as_deref() else {
        return Ok(());
    };
    let supplied = match headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        Some(value) => value.strip_prefix("Bearer ").unwrap_or(value),
        None => return Err(AuthFailure::Missing),
    };
    if constant_time_eq(supplied.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        state.metrics.error();
        Err(AuthFailure::Invalid)
    }
}

fn authorize(headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    check_credential(headers, state, true).map_err(|_| ApiError::unauthorized())
}

fn parse_session_id(value: &str) -> Result<SessionId, ApiError> {
    SessionId::new(value).map_err(|_| ApiError::bad_request())
}

pub(crate) fn internal_auth() -> AuthContext {
    // The application id is a constant that was validated when the program
    // was compiled, so this path cannot fail.
    AuthContext::from_validated(
        nexora_common::ApplicationId::from_static("nexora-server"),
        vec![
            "media.local".to_string(),
            "radio.public".to_string(),
            "http.stream".to_string(),
        ],
    )
}

/// Registers every source adapter NEXORA ships.
///
/// The token-gated providers share one token store: a provider without a
/// token still registers and reports `Unauthorized` until one is supplied,
/// so the rest of the surface stays reachable.
pub fn register_builtin_sources(
    sources: &SourceRegistry,
    tokens: std::sync::Arc<nexora_source_universal::oauth::TokenStore>,
) {
    use nexora_source_universal::apple::AppleMusicSource;
    use nexora_source_universal::bandcamp::BandcampSource;
    use nexora_source_universal::deezer::DeezerSource;
    use nexora_source_universal::soundcloud::SoundcloudSource;
    use nexora_source_universal::spotify::SpotifySource;
    use nexora_source_universal::tidal::TidalSource;
    use nexora_source_universal::transport::InternetArchiveSource;
    use nexora_source_universal::youtube::YouTubeSource;

    let _ = sources.register(Box::new(InternetArchiveSource::new()));
    let _ = sources.register(Box::new(BandcampSource::new()));
    let _ = sources.register(Box::new(YouTubeSource::new(tokens.get("youtube"))));
    let _ = sources.register(Box::new(AppleMusicSource::new(std::sync::Arc::clone(
        &tokens,
    ))));
    let _ = sources.register(Box::new(DeezerSource::new(std::sync::Arc::clone(&tokens))));
    let _ = sources.register(Box::new(SoundcloudSource::new(std::sync::Arc::clone(
        &tokens,
    ))));
    let _ = sources.register(Box::new(SpotifySource::new(std::sync::Arc::clone(&tokens))));
    let _ = sources.register(Box::new(TidalSource::new(std::sync::Arc::clone(&tokens))));
}

/// Loads session state from a snapshot, or starts empty when no snapshot exists.
pub async fn load_sessions(path: &FsPath) -> NexoraResult<Arc<SessionManager>> {
    if !path.exists() {
        return Ok(Arc::new(SessionManager::new()));
    }
    Ok(Arc::new(
        nexora_session::durable::load_from_file(path).await?,
    ))
}

/// Writes the current session state to a snapshot file.
pub async fn save_sessions(path: &FsPath, sessions: &SessionManager) -> NexoraResult<()> {
    nexora_session::durable::save_to_file(sessions, path).await
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
        .route("/metrics", get(prometheus_metrics))
        .route("/ws/v1/sessions/{id}", get(websocket_events))
        .route("/ws/v1/sessions/{id}/audio", get(websocket_audio))
        .merge(nexora::router())
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
        "nexora_compatible": true,
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
    start_playback(&state, &session_id, &request.track_id).await?;
    Ok(Json(json!({ "status": "playing" })))
}

/// Resolves a track and starts the playback task that produces its audio.
///
/// Both the native API and the Nexora nexora player routes start playback the
/// same way, so the resolution and the pacing task live here.
pub(crate) async fn start_playback(
    state: &AppState,
    session_id: &SessionId,
    track_id: &TrackId,
) -> Result<(), ApiError> {
    let resolved = state
        .sources
        .resolve(&TrackReference::new(track_id.as_str()), &internal_auth())
        .await?;
    // A local file plays from disk; an HTTP source is fetched into a
    // temporary file first, so the decoder reads it the same way. The
    // downloaded file is owned by the playback task, so it is removed when the
    // task ends, is replaced or is cancelled.
    let (path, media) = match resolved {
        ResolvedSource::LocalFile { path } => (path, None),
        ResolvedSource::Stream { url } => {
            let media = download_source_to_temp(&url).await.map_err(|error| {
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    error.to_string(),
                    "/api/v1/sessions/play",
                )
            })?;
            let path = media.path().to_path_buf();
            (path, Some(media))
        }
    };
    let format = state.sessions.output_format(session_id).await?;

    // Stop any existing playback task for this session before starting a new
    // one: a nexora session id is the native session id, so a PATCH replacing the
    // track would otherwise spawn a second run_playback task.
    state.sessions.abort_playback_task(session_id).await;

    let (audio_tx, _) = broadcast::channel(64);
    state
        .sessions
        .audio_registry()
        .register(session_id, audio_tx.clone())
        .await;
    let (control_tx, control_rx) = watch::channel(PlaybackSignal::Resume);
    state
        .sessions
        .attach_playback(session_id, control_tx, audio_tx)
        .await?;
    state
        .sessions
        .play(
            session_id,
            PlayRequest {
                track_id: track_id.clone(),
                start_position_ms: 0,
            },
        )
        .await?;

    let sessions = Arc::clone(&state.sessions);
    let metrics = Arc::clone(&state.metrics);
    let sources = Arc::clone(&state.sources);
    let nexora_sessions = Arc::clone(&state.nexora_sessions);
    let player = nexora_player_for(&nexora_sessions, session_id, track_id).await;
    // M5: the UDP voice socket belongs to the player, not to the track. The
    // reference holds one media connection per player and closes it when the
    // player is destroyed, so a track change keeps the same socket.
    let socket = match player.as_ref().map(|player| player.guild_id.clone()) {
        Some(guild_id) => voice_socket_for(state, session_id.as_str(), &guild_id).await,
        None => None,
    };
    let filters = player
        .as_ref()
        .map(|p| p.filters.clone())
        .unwrap_or_default();
    let voice_endpoint = player.as_ref().and_then(|p| p.voice.endpoint.clone());
    let voice_key = player
        .as_ref()
        .and_then(|p| p.voice.token.as_deref())
        .map(xsecret_key);
    let handle = tokio::spawn(run_playback(
        session_id.clone(),
        track_id.clone(),
        path,
        media,
        format,
        control_rx,
        PlaybackContext {
            sessions,
            metrics,
            sources,
            nexora_sessions,
            filters,
            voice_socket: socket,
            voice_endpoint,
            voice_key,
        },
    ));
    // Store the handle so a later replacement can abort this task.
    state.sessions.abort_playback_task(session_id).await;
    state
        .sessions
        .store_playback_task(session_id, Arc::new(handle))
        .await;
    Ok(())
}

/// Looks up the nexora player that owns a native session.
///
/// The nexora session id and the native session id are the same string, so the
/// nexora player for a playback task is found by looking the session id up in
/// the nexora session manager.
/// Looks up the nexora player that is playing a given track.
///
/// The nexora session id and the native session id are the same string, so the
/// nexora player for a playback task is found by looking the session id up in
/// the nexora session manager.
///
/// A session holds one player per guild, so the player is matched on the
/// track it is playing rather than taken as the first one: the events carry
/// that player's guild id, and picking the wrong player would report another
/// guild's track.
async fn nexora_player_for(
    nexora_sessions: &Arc<NexoraSessionManager>,
    session_id: &SessionId,
    track_id: &TrackId,
) -> Option<NexoraPlayerRecord> {
    let players = nexora_sessions.players(session_id.as_str()).await?;
    players.into_iter().find_map(|player| {
        player
            .track
            .as_ref()
            .is_some_and(|track| track.info.identifier == track_id.as_str())
            .then_some(player)
    })
}

/// Starts the per-player `playerUpdate` task.
///
/// The reference schedules this task when a track starts and cancels it when
/// the track ends, so a player only produces updates while it has something
/// to play (bytecode evidence on `NexoraPlayer.onTrackStart` and
/// `onTrackEnd` in the shipped `Nexora.jar`).
///
/// The first frame is sent immediately, matching the reference's
/// `scheduleAtFixedRate(…, 0, interval, SECONDS)` initial delay of zero. The
/// task is skipped entirely while the player is paused — the reference returns
/// before it builds the frame.
fn start_player_updates(
    sessions: Arc<SessionManager>,
    nexora_sessions: Arc<NexoraSessionManager>,
    session_id: SessionId,
    guild_id: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(PLAYER_UPDATE_INTERVAL_SECONDS));
        loop {
            // The interval's first tick is immediate, matching the
            // reference's `scheduleAtFixedRate(…, 0, interval, SECONDS)`
            // initial delay of zero.
            ticker.tick().await;
            let Ok(state) = sessions.get(&session_id).await else {
                break;
            };
            if state.status == "paused" {
                continue;
            }
            nexora_sessions.emit_player_update(NexoraPlayerUpdateFrame {
                state: NexoraPlayerState {
                    time: nexora_common::TimestampMs::now().as_millis() as i64,
                    position: state.position_ms as i64,
                    connected: false,
                    ping: -1,
                },
                guild_id: guild_id.clone(),
            });
            // The player record carries the position too, so the player
            // endpoints report where the track actually is — and a resumed
            // session picks that position up.
            nexora_sessions
                .update_player(session_id.as_str(), &guild_id, |player| {
                    player.position_ms = state.position_ms;
                })
                .await;
        }
    })
}

/// Publishes the `TrackEndEvent` for a session's nexora player.
async fn emit_track_end(
    nexora_sessions: &Arc<NexoraSessionManager>,
    session_id: &SessionId,
    track_id: &TrackId,
    reason: NexoraTrackEndReason,
) {
    let Some(player) = nexora_player_for(nexora_sessions, session_id, track_id).await else {
        return;
    };
    let Some(track) = player.track.clone() else {
        return;
    };
    nexora_sessions.emit(NexoraEventMessage::TrackEnd {
        guild_id: player.guild_id.clone(),
        track: Box::new(track),
        reason,
    });
}

/// Publishes the `TrackExceptionEvent` for a session's nexora player.
async fn emit_track_exception(
    nexora_sessions: &Arc<NexoraSessionManager>,
    session_id: &SessionId,
    track_id: &TrackId,
    message: &str,
    severity: NexoraErrorSeverity,
) {
    let Some(player) = nexora_player_for(nexora_sessions, session_id, track_id).await else {
        return;
    };
    let Some(track) = player.track.clone() else {
        return;
    };
    nexora_sessions.emit(NexoraEventMessage::TrackException {
        guild_id: player.guild_id.clone(),
        track: Box::new(track),
        exception: NexoraException {
            message: message.to_owned(),
            severity,
            cause: None,
            cause_stack_trace: None,
        },
    });
}

async fn prepare_packets(
    path: &FsPath,
    format: &AudioFormat,
    volume: u8,
    filters: &NexoraFilters,
) -> NexoraResult<Vec<Vec<u8>>> {
    let path = path.to_path_buf();
    let sample_rate = format.sample_rate;
    let channels = format.channels;
    let filters = filters.clone();
    tokio::task::spawn_blocking(move || {
        encode_file_packets(&path, sample_rate, channels, volume, &filters)
    })
    .await
    .map_err(|_| NexoraError::service("playback task failed"))?
}

/// The state a playback task shares with the server.
struct PlaybackContext {
    sessions: Arc<SessionManager>,
    metrics: Arc<RuntimeMetrics>,
    sources: Arc<SourceRegistry>,
    nexora_sessions: Arc<NexoraSessionManager>,
    filters: NexoraFilters,
    /// The session's UDP voice socket, if one could be bound.
    ///
    /// The socket is dropped when the playback task ends, closing the port.
    voice_socket: Option<Arc<tokio::net::UdpSocket>>,
    /// The Discord voice gateway endpoint to send Opus packets to, if any.
    voice_endpoint: Option<String>,
    /// The XSecret key derived from the voice session token, if any.
    voice_key: Option<[u8; 16]>,
}

/// Resolves a voice endpoint to the UDP address the RTP packets go to.
///
/// The lookup happens once per playback task: a DNS lookup per packet would
/// dominate the cost of sending a 20 ms frame.
async fn resolve_voice_target(endpoint: Option<&str>) -> Option<std::net::SocketAddr> {
    let endpoint = endpoint?;
    let Some((host, port)) = parse_voice_endpoint(endpoint) else {
        tracing::warn!(%endpoint, "the voice endpoint is not a usable address");
        return None;
    };
    let addresses = tokio::net::lookup_host((host, port)).await;
    match addresses {
        Ok(mut addresses) => addresses.next().or_else(|| {
            tracing::warn!(%host, %port, "the voice host has no address");
            None
        }),
        Err(error) => {
            // A voice target that cannot be resolved means no packet is ever
            // sent, so it has to be visible.
            tracing::warn!(%host, %port, %error, "the voice host cannot be resolved");
            None
        }
    }
}

async fn run_playback(
    session_id: SessionId,
    track_id: TrackId,
    mut path: PathBuf,
    mut media: Option<DownloadedMedia>,
    format: AudioFormat,
    mut control: watch::Receiver<PlaybackSignal>,
    context: PlaybackContext,
) {
    let PlaybackContext {
        sessions,
        metrics,
        sources,
        nexora_sessions,
        filters,
        voice_socket,
        voice_endpoint,
        voice_key,
    } = context;
    let mut volume = sessions
        .get(&session_id)
        .await
        .map(|state| state.volume)
        .unwrap_or(100);
    // The voice stream's SSRC is derived from the session id, so it stays
    // stable for the life of this playback task.
    let mut voice_stream = VoiceStream::new(derive_ssrc(session_id.as_str()));
    let voice_target = resolve_voice_target(voice_endpoint.as_deref()).await;
    let mut packets = match prepare_packets(&path, &format, volume, &filters).await {
        Ok(packets) => packets,
        Err(_) => {
            let _ = sessions.fail(&session_id, "source decode failed").await;
            emit_track_exception(
                &nexora_sessions,
                &session_id,
                &track_id,
                "source decode failed",
                NexoraErrorSeverity::Fault,
            )
            .await;
            sessions.detach_playback(&session_id).await;
            sessions.audio_registry().deregister(&session_id).await;
            return;
        }
    };
    let mut index = 0usize;
    let mut position_ms = 0u64;
    let mut paused = false;
    let mut ticker = tokio::time::interval(Duration::from_millis(20));

    /// Emit `TrackStartEvent` for the currently playing track, if any.
    ///
    /// The reference emits the event when a track begins (bytecode evidence on
    /// `player/EventEmitter.onTrackStart` in the shipped `Nexora.jar`).
    async fn emit_track_start(
        nexora_sessions: &Arc<NexoraSessionManager>,
        session_id: &SessionId,
        track_id: &TrackId,
    ) {
        let Some(player) = nexora_player_for(nexora_sessions, session_id, track_id).await else {
            return;
        };
        let Some(track) = player.track.clone() else {
            return;
        };
        nexora_sessions.emit(NexoraEventMessage::TrackStart {
            guild_id: player.guild_id.clone(),
            track: Box::new(track),
        });
        // The stream metadata event follows the start, so the reference's
        // player-created-then-track-start order stays unbroken.
        nexora_sessions.emit(NexoraEventMessage::StreamMetadata {
            guild_id: player.guild_id.clone(),
            track_id: track_id.as_str().to_owned(),
            title: player
                .track
                .as_ref()
                .map(|t| t.info.title.clone())
                .unwrap_or_default(),
            source_name: player
                .track
                .as_ref()
                .map(|t| t.info.source_name.clone())
                .unwrap_or_default(),
            length_ms: player.track.as_ref().map(|t| t.info.length).unwrap_or(0),
        });
    }

    emit_track_start(&nexora_sessions, &session_id, &track_id).await;
    let update_task = start_player_updates(
        Arc::clone(&sessions),
        Arc::clone(&nexora_sessions),
        session_id.clone(),
        nexora_player_for(&nexora_sessions, &session_id, &track_id)
            .await
            .map(|player| player.guild_id.clone())
            .unwrap_or_default(),
    );

    loop {
        tokio::select! {
            changed = control.changed() => {
                if changed.is_err() {
                    break;
                }
                let signal = *control.borrow_and_update();
                match signal {
                    PlaybackSignal::Pause => paused = true,
                    PlaybackSignal::Resume => paused = false,
                    PlaybackSignal::Stop => break,
                    PlaybackSignal::Seek(position) => {
                        position_ms = position;
                        index = (position / 20) as usize;
                        let _ = sessions.update_position(&session_id, position_ms).await;
                    }
                    PlaybackSignal::SetVolume(next) => {
                        if next == volume {
                            continue;
                        }
                        volume = next;
                        if let Ok(reencoded) = prepare_packets(&path, &format, volume, &filters).await {
                            packets = reencoded;
                            index = (position_ms / 20) as usize;
                        }
                    }
                }
            }
            _ = ticker.tick() => {
                if paused {
                    continue;
                }
                let Some(payload) = packets.get(index).cloned() else {
                    // The track ended: advance to the next queued track when one exists.
                    let Some(next_track) = sessions.pop_next_queued(&session_id).await.ok().flatten()
                    else {
                        let _ = sessions.finish(&session_id).await;
                        emit_track_end(&nexora_sessions, &session_id, &track_id, NexoraTrackEndReason::Finished).await;
                        break;
                    };
                    let _ = sessions.end_track(&session_id).await;
                    // The queue takes over, which is the gapless transition:
                    // the next track starts without a gap.
                    emit_track_end(
                        &nexora_sessions,
                        &session_id,
                        &track_id,
                        NexoraTrackEndReason::Gapless,
                    )
                    .await;
                    let Ok(ResolvedSource::LocalFile { path: next_path }) = sources
                        .resolve(&TrackReference::new(next_track.as_str()), &internal_auth())
                        .await
                    else {
                        let _ = sessions.fail(&session_id, "source resolve failed").await;
                        emit_track_exception(
                            &nexora_sessions,
                            &session_id,
                            &next_track,
                            "source resolve failed",
                            NexoraErrorSeverity::Fault,
                        )
                        .await;
                        break;
                    };
                    match prepare_packets(&next_path, &format, volume, &filters).await {
                        Ok(next_packets) => {
                            let _ = sessions
                                .play(
                                    &session_id,
                                    PlayRequest {
                                        track_id: next_track.clone(),
                                        start_position_ms: 0,
                                    },
                                )
                                .await;
                            path = next_path;
                            // The previous temporary file is no longer needed.
                            media = None;
                            packets = next_packets;
                            index = 0;
                            position_ms = 0;
                            ticker.reset();
                            emit_track_start(&nexora_sessions, &session_id, &next_track).await;
                        }
                        Err(_) => {
                            let _ = sessions.fail(&session_id, "source decode failed").await;
                            emit_track_exception(
                                &nexora_sessions,
                                &session_id,
                                &next_track,
                                "source decode failed",
                                NexoraErrorSeverity::Fault,
                            )
                            .await;
                            break;
                        }
                    }
                    continue;
                };
                let frame = OpusFrame::packet(index as u64, payload.clone());
                let _ = sessions.audio_registry().send_frame(&session_id, frame).await;
                metrics.streamed(payload.len());
                // M5: frame the Opus packet as RTP, encrypt the payload with
                // XSecret (AES-128-CTR keyed by the voice session token, nonce
                // = the RTP sequence number) and forward it to the Discord
                // voice gateway over UDP. A cipher failure leaves the packet
                // undelivered; audio keeps playing.
                if let Some(key) = voice_key.as_ref()
                    && let (Some(socket), Some(target)) =
                        (voice_socket.as_ref(), voice_target)
                    && let Ok(datagram) = voice_stream.next_packet(key, &payload)
                {
                    let _ = socket.send_to(&datagram, target).await;
                }
                index += 1;
                position_ms += 20;
                let _ = sessions.update_position(&session_id, position_ms).await;
            }
        }
    }

    update_task.abort();
    sessions.detach_playback(&session_id).await;
    sessions.audio_registry().deregister(&session_id).await;
    // Dropping `voice_socket` closes the port when playback ends.
    drop(voice_socket);
    // Dropping the downloaded media removes the temporary file it owns.
    drop(media);
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

async fn prometheus_metrics(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    authorize(&headers, &state)?;
    let body = format!(
        "# TYPE nexora_active_sessions gauge\n\
nexora_active_sessions {}\n\
# TYPE nexora_api_requests_total counter\n\
nexora_api_requests_total {}\n\
# TYPE nexora_search_requests_total counter\n\
nexora_search_requests_total {}\n\
# TYPE nexora_stream_bytes_total counter\n\
nexora_stream_bytes_total {}\n\
# TYPE nexora_api_errors_total counter\n\
nexora_api_errors_total {}\n",
        state.metrics.active_sessions(),
        state.metrics.api_requests.load(Ordering::Relaxed),
        state.metrics.search_requests.load(Ordering::Relaxed),
        state.metrics.stream_bytes.load(Ordering::Relaxed),
        state.metrics.errors.load(Ordering::Relaxed),
    );
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain; version=0.0.4")
        .body(Body::from(body))
        .unwrap())
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

async fn websocket_audio(
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
    let receiver = state.sessions.audio_registry().subscribe(&session_id).await;
    let Some(receiver) = receiver else {
        return ApiError::not_found().into_response();
    };
    upgrade.on_upgrade(move |socket| forward_audio(socket, receiver))
}

async fn forward_audio(mut socket: WebSocket, mut receiver: broadcast::Receiver<OpusFrame>) {
    loop {
        match receiver.recv().await {
            Ok(frame) => {
                if socket
                    .send(Message::Binary(frame.payload.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => break,
        }
    }
}

/// How often the source health is checked.
pub const SOURCE_HEALTH_INTERVAL_SECONDS: u64 = 30;

/// Watches the sources' health and reports what changed.
///
/// The first check sends the whole snapshot; after that only the sources
/// whose health moved send a change event.
pub fn spawn_source_health_watch(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut previous: Vec<nexora_source_api::SourceHealth> = Vec::new();
        let mut ticker = tokio::time::interval(Duration::from_secs(SOURCE_HEALTH_INTERVAL_SECONDS));
        loop {
            ticker.tick().await;
            let reports = state.sources.health_all(&internal_auth()).await;
            if previous.is_empty() {
                state
                    .nexora_sessions
                    .emit(NexoraEventMessage::SourceHealthSnapshot {
                        sources: reports.iter().map(health_to_wire).collect(),
                    });
            } else {
                for report in &reports {
                    let before = previous
                        .iter()
                        .find(|old| old.source_id == report.source_id);
                    let changed = before.is_none_or(|old| {
                        old.healthy != report.healthy || old.detail != report.detail
                    });
                    if changed {
                        state
                            .nexora_sessions
                            .emit(NexoraEventMessage::SourceHealthChange {
                                source_id: report.source_id.clone(),
                                healthy: report.healthy,
                                detail: report.detail.clone(),
                            });
                    }
                }
            }
            previous = reports;
        }
    })
}

/// Maps a source layer's health report onto the wire type.
fn health_to_wire(report: &nexora_source_api::SourceHealth) -> nexora_protocol::NexoraSourceHealth {
    nexora_protocol::NexoraSourceHealth {
        source_id: report.source_id.clone(),
        healthy: report.healthy,
        detail: report.detail.clone(),
        checked_at: report.checked_at.as_millis(),
    }
}

pub(crate) fn process_start_ms() -> u64 {
    static START: OnceLock<u64> = OnceLock::new();
    *START.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    })
}

/// Milliseconds since the process started.
pub(crate) fn process_uptime_ms() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    now.saturating_sub(process_start_ms())
}
