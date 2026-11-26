//! The Nexora nexora surface.
//!
//! The route list below is the one the shipped `Nexora.jar` (4.2.2)
//! actually serves, read from its handler annotations:
//!
//! | Method | Path |
//! | --- | --- |
//! | GET | `/version` |
//! | GET | `/nexora/info` |
//! | GET | `/nexora/stats` |
//! | GET | `/nexora/routeplanner/status` |
//! | POST | `/nexora/routeplanner/free/address` |
//! | POST | `/nexora/routeplanner/free/all` |
//! | GET | `/nexora/loadtracks` |
//! | GET | `/nexora/decodetrack` |
//! | POST | `/nexora/decodetracks` |
//! | POST | `/nexora/sessions` |
//! | PATCH | `/nexora/sessions/{sessionId}` |
//! | GET | `/nexora/sessions/{sessionId}/players` |
//! | GET | `/nexora/sessions/{sessionId}/players/{guildId}` |
//! | PATCH | `/nexora/sessions/{sessionId}/players/{guildId}` |
//! | DELETE | `/nexora/sessions/{sessionId}/players/{guildId}` |
//! | WS | `/nexora/websocket` |
//!
//! Nexora nexora has **no** queue routes: the queue is not part of its REST
//! surface, so NEXORA does not invent one.
//!
//! The Nexora extras NEXORA also serves are marked as such below.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use nexora_audio_types::AudioFormat;
use nexora_common::{SessionId, TrackId};
use nexora_protocol::{
    NexoraErrorEnvelope, NexoraEventMessage, NexoraFilters, NexoraGit, NexoraInfo,
    NexoraLoadResult, NexoraPlayer, NexoraPlayerUpdate, NexoraPlayers, NexoraPlugins, NexoraReady,
    NexoraSession, NexoraSessionUpdate, NexoraStatsFrame, NexoraTrack, NexoraTrackEndReason,
    NexoraTrackInfo, NexoraVersion, decode_track, encode_track,
};
use nexora_source_api::{ResolvedSource, TrackMetadata, TrackReference};
use nexora_source_framework::rank_candidates;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use crate::{AppState, internal_auth, start_playback};

/// The reference pushes stats once a minute.
const STATS_INTERVAL_SECONDS: u64 = 60;

/// An error in the reference's HTTP error shape.
#[derive(Debug)]
pub struct NexoraApiError {
    status: StatusCode,
    message: String,
    path: String,
}

impl NexoraApiError {
    fn new(status: StatusCode, message: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            path: path.into(),
        }
    }

    fn bad_request(message: impl Into<String>, path: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message, path)
    }

    fn not_found(message: impl Into<String>, path: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message, path)
    }
}

impl IntoResponse for NexoraApiError {
    fn into_response(self) -> Response {
        let status = self.status;
        let envelope = NexoraErrorEnvelope::new(
            status.as_u16(),
            status.canonical_reason().unwrap_or("Error"),
            self.message,
            self.path,
        );
        (status, Json(envelope)).into_response()
    }
}

/// Authorises a nexora request.
///
/// The reference compares the `Authorization` header with the configured
/// password by string equality: a missing header is a 401 and a wrong one is
/// a 403. NEXORA also accepts the `Bearer ` prefix its own API uses.
fn authorize(headers: &HeaderMap, state: &AppState) -> Result<(), NexoraApiError> {
    let Some(expected) = state.password.as_deref() else {
        return Ok(());
    };
    let supplied = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    match supplied {
        None => Err(NexoraApiError::new(
            StatusCode::UNAUTHORIZED,
            "authorization required",
            "/nexora",
        )),
        Some(value) if value == expected || value.strip_prefix("Bearer ") == Some(expected) => {
            Ok(())
        }
        Some(_) => Err(NexoraApiError::new(
            StatusCode::FORBIDDEN,
            "authorization failed",
            "/nexora",
        )),
    }
}

/// Authorises the nexora WebSocket handshake.
///
/// The reference's handshake interceptor has a single failure status — 401 —
/// for both a missing and a wrong `Authorization` header, unlike its REST
/// filter, which answers 401 for a missing header and 403 for a wrong one.
fn authorize_handshake(headers: &HeaderMap, state: &AppState) -> Result<(), NexoraApiError> {
    let Some(expected) = state.password.as_deref() else {
        return Ok(());
    };
    let supplied = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    match supplied {
        None => Err(NexoraApiError::new(
            StatusCode::UNAUTHORIZED,
            "authorization required",
            "/nexora/websocket",
        )),
        Some(value) if value == expected || value.strip_prefix("Bearer ") == Some(expected) => {
            Ok(())
        }
        Some(_) => Err(NexoraApiError::new(
            StatusCode::UNAUTHORIZED,
            "authorization failed",
            "/nexora/websocket",
        )),
    }
}

/// The Nexora nexora routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/version", get(version))
        .route("/nexora/info", get(info))
        .route("/nexora/stats", get(stats))
        .route("/nexora/routeplanner/status", get(routeplanner_status))
        .route(
            "/nexora/routeplanner/free/address",
            post(routeplanner_free_address),
        )
        .route("/nexora/routeplanner/free/all", post(routeplanner_free_all))
        .route("/nexora/loadtracks", get(loadtracks))
        .route("/nexora/decodetrack", get(decode_track_route))
        .route("/nexora/decodetracks", post(decode_tracks))
        .route("/nexora/sessions", post(create_session))
        .route(
            "/nexora/sessions/{id}",
            get(get_session).patch(patch_session),
        )
        .route("/nexora/sessions/{id}/players", get(players))
        .route(
            "/nexora/sessions/{id}/players/{guild}",
            get(player).patch(patch_player).delete(delete_player),
        )
        .route("/nexora/websocket", get(websocket))
        // Nexora extras.
        .route("/nexora/connection", get(connection))
        .route("/nexora/loadlyrics", get(load_lyrics))
        .route("/nexora/loadchapters", get(load_chapters))
        .route("/nexora/trackstream/{encoded}", get(trackstream))
}

/// `GET /version` — the bare version string as plain text.
async fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// `GET /nexora/info` — the node's identity and compatibility surface.
async fn info(State(state): State<AppState>) -> Json<Value> {
    let build_time = crate::process_start_ms() as i64;
    let info = NexoraInfo {
        name: "NEXORA".to_owned(),
        version: NexoraVersion::nexora(),
        build_time,
        git: NexoraGit {
            branch: "main".to_owned(),
            commit: "nexora".to_owned(),
            commit_time: build_time,
        },
        jvm: Some("rust-native".to_owned()),
        lavaplayer: Some("nexora-audio-engine".to_owned()),
        source_managers: state
            .sources
            .source_ids()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        filters: NexoraFilters::default()
            .enabled_names()
            .into_iter()
            .map(String::from)
            .collect(),
        plugins: NexoraPlugins::default(),
    };
    Json(serde_json::to_value(info).unwrap_or_else(|_| json!({})))
}

/// The stats payload the reference serves.
fn stats_payload(state: &AppState) -> NexoraStatsFrame {
    NexoraStatsFrame {
        frame_stats: None,
        players: state.metrics.active_sessions() as i64,
        playing_players: state.metrics.active_sessions() as i64,
        uptime: crate::process_uptime_ms() as i64,
        memory: json!({
            "free": 0,
            "used": 0,
            "allocated": 0,
            "reservable": 0,
        }),
        cpu: json!({
            "cores": std::thread::available_parallelism()
                .map(|value| value.get() as i64)
                .unwrap_or(1),
            "systemLoad": 0.0,
            "nexoraLoad": 0.0,
        }),
    }
}

/// `GET /nexora/stats` — the reference's stats block.
///
/// The reference's REST payload has no `op` field; only the WebSocket
/// `stats` event carries one.
async fn stats(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    Ok(Json(
        serde_json::to_value(stats_payload(&state)).unwrap_or(Value::Null),
    ))
}

/// `GET /nexora/routeplanner/status` — 204 when no route planner is configured.
async fn routeplanner_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, NexoraApiError> {
    authorize(&headers, &state)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `POST /nexora/routeplanner/free/address` — 204; NEXORA has no route planner.
async fn routeplanner_free_address(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, NexoraApiError> {
    authorize(&headers, &state)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `POST /nexora/routeplanner/free/all` — 204; NEXORA has no route planner.
async fn routeplanner_free_all(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, NexoraApiError> {
    authorize(&headers, &state)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    identifier: String,
}

#[derive(Debug, Deserialize)]
struct DecodeQuery {
    #[serde(rename = "encodedTrack")]
    encoded_track: Option<String>,
    track: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EncodedTracks {
    tracks: Vec<String>,
}

/// Builds the reference's track object from a source's metadata.
fn build_track(metadata: &TrackMetadata) -> Result<NexoraTrack, NexoraApiError> {
    let info = NexoraTrackInfo {
        identifier: metadata.track_id.as_str().to_owned(),
        is_seekable: metadata.duration_ms.is_some(),
        author: metadata
            .artist
            .clone()
            .unwrap_or_else(|| "unknown".to_owned()),
        length: metadata.duration_ms.unwrap_or(0),
        is_stream: false,
        position: 0,
        title: metadata.title.clone(),
        uri: None,
        artwork_url: None,
        isrc: None,
        source_name: metadata.source_id.clone(),
        filters: None,
    };
    let encoded = encode_track(&info).map_err(|error| {
        NexoraApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
            "/nexora/loadtracks",
        )
    })?;
    Ok(NexoraTrack {
        encoded,
        info,
        plugin_info: json!({}),
        user_data: json!({}),
    })
}

/// `GET /nexora/loadtracks` — resolve an identifier into a load result.
///
/// The reference always answers HTTP 200: a failure is reported in the body.
async fn loadtracks(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    let results = state
        .sources
        .search(&query.identifier, &internal_auth())
        .await
        .map_err(|error| {
            NexoraApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                "/nexora/loadtracks",
            )
        })?;
    // Rank the candidates before the cut: an exact title match comes first.
    let mut results = rank_candidates(results, &query.identifier, None);
    results.truncate(25);
    if results.is_empty() {
        return Ok(Json(
            serde_json::to_value(NexoraLoadResult::empty()).unwrap_or(Value::Null),
        ));
    }
    let tracks: Vec<NexoraTrack> = results
        .iter()
        .map(build_track)
        .collect::<Result<Vec<NexoraTrack>, NexoraApiError>>()?;
    Ok(Json(
        serde_json::to_value(NexoraLoadResult::search(tracks)).unwrap_or(Value::Null),
    ))
}

/// `GET /nexora/decodetrack` — decode one encoded track.
///
/// The reference echoes the input encoded string rather than re-encoding it.
async fn decode_track_route(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<DecodeQuery>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    let encoded = query.encoded_track.or(query.track).ok_or_else(|| {
        NexoraApiError::bad_request("No track to decode provided", "/nexora/decodetrack")
    })?;
    let info = decode_track(&encoded)
        .map_err(|error| NexoraApiError::bad_request(error.to_string(), "/nexora/decodetrack"))?;
    let track = NexoraTrack {
        encoded,
        info,
        plugin_info: json!({}),
        user_data: json!({}),
    };
    Ok(Json(serde_json::to_value(track).unwrap_or(Value::Null)))
}

/// `POST /nexora/decodetracks` — decode a batch of encoded tracks.
async fn decode_tracks(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<EncodedTracks>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    if request.tracks.is_empty() {
        return Err(NexoraApiError::bad_request(
            "No tracks to decode provided",
            "/nexora/decodetracks",
        ));
    }
    let mut tracks = Vec::with_capacity(request.tracks.len());
    for encoded in request.tracks {
        let info = decode_track(&encoded).map_err(|error| {
            NexoraApiError::bad_request(error.to_string(), "/nexora/decodetracks")
        })?;
        tracks.push(NexoraTrack {
            encoded,
            info,
            plugin_info: json!({}),
            user_data: json!({}),
        });
    }
    Ok(Json(json!({ "tracks": tracks })))
}

/// `POST /nexora/sessions` — create a session and return its id.
async fn create_session(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<Value>), NexoraApiError> {
    authorize(&headers, &state)?;
    let session_id = state.nexora_sessions.create_session().await;
    let native = SessionId::new(session_id.clone()).map_err(|_| {
        NexoraApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session id is not usable",
            "/nexora/sessions",
        )
    })?;
    state
        .sessions
        .create_with_id(
            native,
            AudioFormat::new(48_000, 2, nexora_audio_types::AudioCodec::Opus),
        )
        .await
        .map_err(|error| {
            NexoraApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
                "/nexora/sessions",
            )
        })?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "sessionId": session_id })),
    ))
}

/// `PATCH /nexora/sessions/{sessionId}` — the session's resume behaviour.
async fn patch_session(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(update): Json<NexoraSessionUpdate>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    let (resuming, timeout) = state
        .nexora_sessions
        .set_resuming(&id, update.resuming, update.timeout)
        .await
        .ok_or_else(|| {
            NexoraApiError::not_found("Session not found", format!("/nexora/sessions/{id}"))
        })?;
    Ok(Json(
        serde_json::to_value(NexoraSession { resuming, timeout }).unwrap_or(Value::Null),
    ))
}

/// `GET /nexora/sessions/{sessionId}` — the session's resume behaviour.
async fn get_session(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    let session = state.nexora_sessions.session(&id).await.ok_or_else(|| {
        NexoraApiError::not_found("Session not found", format!("/nexora/sessions/{id}"))
    })?;
    Ok(Json(
        serde_json::to_value(NexoraSession {
            resuming: session.resuming,
            timeout: session.resume_timeout,
        })
        .unwrap_or(Value::Null),
    ))
}

/// `GET /nexora/sessions/{sessionId}/players` — every player of the session.
async fn players(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    let players = state.nexora_sessions.players(&id).await.ok_or_else(|| {
        NexoraApiError::not_found(
            "Session not found",
            format!("/nexora/sessions/{id}/players"),
        )
    })?;
    let players: Vec<NexoraPlayer> = players.iter().map(|player| player.to_player()).collect();
    Ok(Json(
        serde_json::to_value(NexoraPlayers { players }).unwrap_or(Value::Null),
    ))
}

/// `GET /nexora/sessions/{sessionId}/players/{guildId}` — one player.
async fn player(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((id, guild)): Path<(String, String)>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    let player = state
        .nexora_sessions
        .player(&id, &guild)
        .await
        .ok_or_else(|| {
            NexoraApiError::not_found(
                "Player not found",
                format!("/nexora/sessions/{id}/players/{guild}"),
            )
        })?;
    Ok(Json(
        serde_json::to_value(player.to_player()).unwrap_or(Value::Null),
    ))
}

#[derive(Debug, Deserialize)]
struct PlayerQuery {
    #[serde(default)]
    noreplace: bool,
}

/// `DELETE /nexora/sessions/{sessionId}/players/{guildId}` — destroy a player.
async fn delete_player(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((id, guild)): Path<(String, String)>,
) -> Result<Response, NexoraApiError> {
    authorize(&headers, &state)?;
    if state.nexora_sessions.session(&id).await.is_none() {
        return Err(NexoraApiError::not_found(
            "Session not found",
            format!("/nexora/sessions/{id}/players/{guild}"),
        ));
    }
    state.nexora_sessions.remove_player(&id, &guild).await;
    state
        .nexora_sessions
        .emit(NexoraEventMessage::PlayerDestroyed {
            guild_id: guild.to_owned(),
        });
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `PATCH /nexora/sessions/{sessionId}/players/{guildId}` — update a player.
///
/// The field-application order follows the reference: voice, then paused, user
/// data, volume, position, end time and filters, and finally a new track.
async fn patch_player(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((id, guild)): Path<(String, String)>,
    Query(query): Query<PlayerQuery>,
    Json(update): Json<NexoraPlayerUpdate>,
) -> Result<Json<Value>, NexoraApiError> {
    let path = format!("/nexora/sessions/{id}/players/{guild}");
    authorize(&headers, &state)?;
    if state.nexora_sessions.session(&id).await.is_none() {
        return Err(NexoraApiError::not_found("Session not found", path.clone()));
    }

    let track_block = update.track.as_ref();
    let new_track_requested =
        track_block.is_some() || update.encoded_track.is_some() || update.identifier.is_some();
    if track_block.is_some() && (update.encoded_track.is_some() || update.identifier.is_some()) {
        return Err(NexoraApiError::bad_request(
            "Cannot specify both track and encodedTrack/identifier",
            path.clone(),
        ));
    }
    if let Some(track) = track_block
        && track.encoded.is_some()
        && track.identifier.is_some()
    {
        return Err(NexoraApiError::bad_request(
            "Cannot specify both encodedTrack and identifier",
            path.clone(),
        ));
    }
    if let Some(Some(end_time)) = update.end_time
        && end_time == 0
    {
        return Err(NexoraApiError::bad_request(
            "End time must be greater than 0",
            path.clone(),
        ));
    }
    if let Some(voice) = &update.voice
        && !voice.is_complete()
    {
        return Err(NexoraApiError::bad_request(
            "token, endpoint, sessionId and channelId must be provided in voice state",
            path.clone(),
        ));
    }

    // The identifier the request asks for, if any.
    let requested_identifier = track_block
        .and_then(|track| track.identifier.clone())
        .or_else(|| update.identifier.clone());
    let requested_encoded = track_block
        .and_then(|track| track.encoded.clone())
        .or_else(|| update.encoded_track.clone());

    let current = state.nexora_sessions.player(&id, &guild).await;
    // The reference skips the whole request and returns the current state when
    // `noreplace` is set and the player is already playing.
    if query.noreplace
        && current
            .as_ref()
            .is_some_and(|player| player.track.is_some())
        && let Some(player) = current
    {
        return Ok(Json(
            serde_json::to_value(player.to_player()).unwrap_or(Value::Null),
        ));
    }

    // Resolve the new track before touching any state.
    let mut new_track: Option<NexoraTrack> = None;
    let mut stop_requested = false;
    if requested_encoded.is_some() || requested_identifier.is_some() {
        if let Some(Some(encoded)) = requested_encoded.clone() {
            let info = decode_track(&encoded)
                .map_err(|error| NexoraApiError::bad_request(error.to_string(), path.clone()))?;
            new_track = Some(NexoraTrack {
                encoded,
                info,
                plugin_info: json!({}),
                user_data: json!({}),
            });
        } else if let Some(identifier) = requested_identifier.clone() {
            let results = state
                .sources
                .search(&identifier, &internal_auth())
                .await
                .map_err(|error| NexoraApiError::bad_request(error.to_string(), path.clone()))?;
            let metadata = results.first().ok_or_else(|| {
                NexoraApiError::bad_request("No matches found for identifier", path.clone())
            })?;
            new_track = Some(build_track(metadata)?);
        } else {
            // An explicit null track stops the player.
            stop_requested = true;
        }
    }

    let paused = update.paused.unwrap_or(false);
    let user_data = track_block.and_then(|track| track.user_data.clone());

    // The player before the change, so the Nexora events fire only for a
    // value that actually moved.
    let before = state.nexora_sessions.player(&id, &guild).await;

    let player = state
        .nexora_sessions
        .update_player(&id, &guild, |player| {
            if let Some(voice) = &update.voice {
                player.voice = voice.clone();
            }
            if !new_track_requested {
                if let Some(paused) = update.paused {
                    player.paused = paused;
                }
                if let Some(data) = &user_data {
                    player.user_data = data.clone();
                }
            }
            if let Some(volume) = update.volume {
                player.volume = volume;
            }
            if !new_track_requested && let Some(position) = update.position {
                player.position_ms = position;
            }
            if !new_track_requested && let Some(Some(end_time)) = update.end_time {
                player.end_time_ms = Some(end_time);
            }
            if let Some(filters) = &update.filters {
                player.filters = filters.clone();
            }
            if let Some(track) = &new_track {
                player.track = Some(track.clone());
                player.paused = paused;
                player.position_ms = update.position.unwrap_or(0);
            } else if stop_requested {
                player.track = None;
                player.paused = false;
                player.position_ms = 0;
            }
        })
        .await
        .ok_or_else(|| NexoraApiError::not_found("Session not found", path.clone()))?;

    // The Nexora events report the change the client just made.
    if before.is_none() {
        state
            .nexora_sessions
            .emit(NexoraEventMessage::PlayerCreated {
                guild_id: guild.clone(),
                player: Box::new(player.to_player()),
            });
    }
    if let Some(volume) = update.volume
        && before
            .as_ref()
            .is_none_or(|previous| previous.volume != volume)
    {
        // The volume has to reach the native session, or the audio keeps the
        // old volume while the player reports the new one. The reference
        // passes the value straight through without a range check, so the
        // player reports whatever the client asked for.
        if let Ok(native) = SessionId::new(id.clone())
            && let Ok(level) = u8::try_from(volume.clamp(0, 150))
        {
            let _ = state.sessions.set_volume(&native, level).await;
        }
        state
            .nexora_sessions
            .emit(NexoraEventMessage::VolumeChanged {
                guild_id: guild.clone(),
                volume,
            });
    }
    if let Some(filters) = &update.filters
        && before
            .as_ref()
            .is_none_or(|previous| &previous.filters != filters)
    {
        state
            .nexora_sessions
            .emit(NexoraEventMessage::FiltersChanged {
                guild_id: guild.clone(),
                filters: filters.clone(),
            });
    }
    if let Some(paused) = update.paused
        && !new_track_requested
        && before
            .as_ref()
            .is_none_or(|previous| previous.paused != paused)
    {
        // The pause has to reach the native session, or the audio keeps
        // playing while the player claims to be paused.
        if let Ok(native) = SessionId::new(id.clone()) {
            if paused {
                let _ = state.sessions.pause(&native).await;
            } else {
                let _ = state.sessions.resume(&native).await;
            }
        }
        state.nexora_sessions.emit(NexoraEventMessage::Pause {
            guild_id: guild.clone(),
            paused,
        });
    }
    if let Some(position) = update.position
        && !new_track_requested
        && before
            .as_ref()
            .is_none_or(|previous| previous.position_ms != position)
    {
        if let Ok(native) = SessionId::new(id.clone()) {
            let _ = state.sessions.seek(&native, position).await;
        }
        state.nexora_sessions.emit(NexoraEventMessage::Seek {
            guild_id: guild.clone(),
            position,
            duration: position as i64
                - before
                    .as_ref()
                    .map_or(0, |previous| previous.position_ms as i64),
        });
    }

    // Drive the actual audio for the player that now has a track.
    if let Some(track) = &new_track {
        let native = SessionId::new(id.clone())
            .map_err(|_| NexoraApiError::not_found("Session not found", path.clone()))?;
        let track_id = TrackId::new(track.info.identifier.clone())
            .map_err(|_| NexoraApiError::bad_request("invalid track identifier", path.clone()))?;
        // Stop any existing playback task for this session and report the
        // replaced track, so a PATCH replacing a playing track emits
        // TrackEndEvent(Replaced) and never runs two playback tasks at once.
        state.sessions.abort_playback_task(&native).await;
        if let Some(old_track) = before.as_ref().and_then(|p| p.track.clone()) {
            state.nexora_sessions.emit(NexoraEventMessage::TrackEnd {
                guild_id: guild.clone(),
                track: Box::new(old_track),
                reason: NexoraTrackEndReason::Replaced,
            });
        }
        start_playback(&state, &native, &track_id)
            .await
            .map_err(|error| {
                NexoraApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    error.message,
                    path.clone(),
                )
            })?;
        // The `TrackStartEvent` comes from the playback task, which emits it
        // when the track actually starts — the reference emits it from the
        // audio player's start callback, not from the request that asked for
        // the track.
    } else if stop_requested {
        if let Ok(native) = SessionId::new(id.clone()) {
            let _ = state.sessions.stop(&native).await;
        }
        // The track that was playing is the one that ended, so it is read
        // from the state before the stop cleared it.
        if let Some(track) = before.as_ref().and_then(|player| player.track.clone()) {
            state.nexora_sessions.emit(NexoraEventMessage::TrackEnd {
                guild_id: guild.clone(),
                track: Box::new(track),
                reason: NexoraTrackEndReason::Stopped,
            });
        }
    }

    // If the voice state changed and the player has a track, restart the
    // playback task so Opus packets are encrypted with the new key and sent
    // to the new voice endpoint.
    if before.as_ref().is_some_and(|previous| previous.voice != player.voice)
        && player.track.is_some()
    {
        let native = SessionId::new(id.clone())
            .map_err(|_| NexoraApiError::not_found("Session not found", path.clone()))?;
        let track = player.track.as_ref().unwrap();
        let track_id = TrackId::new(track.info.identifier.clone())
            .map_err(|_| NexoraApiError::bad_request("invalid track identifier", path.clone()))?;
        state.sessions.abort_playback_task(&native).await;
        start_playback(&state, &native, &track_id).await.map_err(|error| {
            NexoraApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                error.message,
                path.clone(),
            )
        })?;
    }

    Ok(Json(
        serde_json::to_value(player.to_player()).unwrap_or(Value::Null),
    ))
}

/// `WS /nexora/websocket` — the session's event stream.
async fn websocket(
    headers: HeaderMap,
    State(state): State<AppState>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if let Err(error) = authorize_handshake(&headers, &state) {
        return error.into_response();
    }
    // The reference requires a `User-Id` header that parses as a non-zero long.
    let user_id = headers
        .get("User-Id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let parsed = user_id.parse::<i64>();
    if parsed.is_err() || parsed == Ok(0) {
        return NexoraApiError::bad_request("User-Id header is required", "/nexora/websocket")
            .into_response();
    }

    // A `Session-Id` header resumes a session when one exists.
    let requested = headers
        .get("Session-Id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let existing = match &requested {
        Some(id) => state.nexora_sessions.session(id).await.is_some(),
        None => false,
    };
    let session_id = match requested {
        Some(id) if state.nexora_sessions.session(&id).await.is_some() => id,
        _ => state.nexora_sessions.create_session().await,
    };
    let resumed = existing;

    let mut response =
        upgrade.on_upgrade(move |socket| run_socket(socket, state, session_id, resumed));
    let resumed_header = if resumed { "true" } else { "false" };
    if let Ok(value) = resumed_header.parse() {
        response.headers_mut().insert("Session-Resumed", value);
    }
    response
}

/// The WebSocket's send loop.
async fn run_socket(mut socket: WebSocket, state: AppState, session_id: String, resumed: bool) {
    let ready = NexoraReady {
        resumed,
        session_id: session_id.clone(),
    };
    if send_frame(&mut socket, &ready.to_frame()).await.is_err() {
        return;
    }

    let mut events = state.nexora_sessions.subscribe();
    let mut stats_interval =
        tokio::time::interval(std::time::Duration::from_secs(STATS_INTERVAL_SECONDS));
    stats_interval.tick().await;

    loop {
        tokio::select! {
            received = events.recv() => {
                match received {
                    Ok(frame) => {
                        if send_frame(&mut socket, &frame.to_frame()).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
            _ = stats_interval.tick() => {
                if send_frame(&mut socket, &stats_payload(&state).to_frame()).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                // The nexora socket is receive-only for events; the reference warns
                // that inbound messages must use the REST API. Voice state
                // updates are the exception: the client sends them over the
                // socket and the server echoes them back.
                match incoming {
                    None => break,
                    Some(Err(_)) => break,
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(value) = serde_json::from_str::<Value>(&text)
                            && let Some(op) = value.get("op").and_then(|v| v.as_str())
                            && (op == "voiceStateUpdate" || op == "4")
                        {
                            let guild_id = value
                                .get("guildId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_owned();
                            let user_id = value
                                .get("userId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_owned();
                            let channel_id = value
                                .get("channelId")
                                .and_then(|v| v.as_str())
                                .map(|v| v.to_owned());
                            let self_deaf = value
                                .get("selfDeaf")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let self_mute = value
                                .get("selfMute")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            // Update the session's voice state for this guild.
                            std::mem::drop(state.nexora_sessions.update_player(
                                &session_id,
                                &guild_id,
                                |player| {
                                    player.voice_channel_id = channel_id.clone();
                                    player.self_deaf = self_deaf;
                                    player.self_mute = self_mute;
                                },
                            ));
                            // Echo back the voice state update event.
                            state.nexora_sessions.emit(NexoraEventMessage::VoiceStateUpdate {
                                guild_id,
                                user_id,
                                channel_id,
                                self_deaf,
                                self_mute,
                            });
                        }
                    }
                    Some(Ok(_)) => continue,
                }
            }
        }
    }

    // The socket is gone: report it to the session's players, the way the
    // reference's `WebSocketClosedEvent` does.
    if let Some(players) = state.nexora_sessions.players(&session_id).await {
        for player in players {
            state
                .nexora_sessions
                .emit(NexoraEventMessage::WebSocketClosed {
                    guild_id: player.guild_id.clone(),
                    code: 1000,
                    reason: "socket closed".to_owned(),
                    by_remote: false,
                });
        }
    }
}

/// Sends one frame as JSON text.
async fn send_frame(socket: &mut WebSocket, frame: &Value) -> Result<(), ()> {
    let Ok(text) = serde_json::to_string(frame) else {
        return Err(());
    };
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| ())
}

/// `GET /nexora/connection` — Nexora's connection diagnostics.
///
/// NEXORA has no voice gateway, so the endpoint reports the reference's
/// disabled payload.
async fn connection(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    Ok(Json(json!({
        "status": "disabled",
        "metrics": null,
        "reason": "connection_manager_unavailable_in_this_process",
    })))
}

#[derive(Debug, Deserialize)]
struct LyricsQuery {
    #[serde(rename = "encodedTrack")]
    encoded_track: Option<String>,
    source: Option<String>,
}

/// `GET /nexora/loadlyrics` — Nexora's lyrics endpoint.
///
/// NEXORA has no lyrics source, so the endpoint answers with an empty
/// payload rather than pretending it found something.
async fn load_lyrics(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<LyricsQuery>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    let Some(encoded) = query.encoded_track else {
        return Err(NexoraApiError::bad_request(
            "Missing encodedTrack parameter.",
            "/nexora/loadlyrics",
        ));
    };
    decode_track(&encoded)
        .map_err(|error| NexoraApiError::bad_request(error.to_string(), "/nexora/loadlyrics"))?;
    Ok(Json(json!({
        "sourceName": query.source.unwrap_or_default(),
        "type": "none",
        "data": null,
    })))
}

/// `GET /nexora/loadchapters` — Nexora's chapters endpoint.
async fn load_chapters(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<LyricsQuery>,
) -> Result<Json<Value>, NexoraApiError> {
    authorize(&headers, &state)?;
    let Some(encoded) = query.encoded_track else {
        return Err(NexoraApiError::bad_request(
            "Missing encodedTrack parameter.",
            "/nexora/loadchapters",
        ));
    };
    decode_track(&encoded)
        .map_err(|error| NexoraApiError::bad_request(error.to_string(), "/nexora/loadchapters"))?;
    Ok(Json(json!([])))
}

/// `GET /nexora/trackstream/{encodedTrack}` — Nexora's track stream.
async fn trackstream(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(encoded): Path<String>,
) -> Result<Response, NexoraApiError> {
    authorize(&headers, &state)?;
    let info = decode_track(&encoded)
        .map_err(|error| NexoraApiError::bad_request(error.to_string(), "/nexora/trackstream"))?;
    let track_id = TrackId::new(info.identifier.clone()).map_err(|_| {
        NexoraApiError::bad_request("invalid track identifier", "/nexora/trackstream")
    })?;
    let resolved = state
        .sources
        .resolve(&TrackReference::new(track_id.as_str()), &internal_auth())
        .await
        .map_err(|error| {
            NexoraApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                error.to_string(),
                "/nexora/trackstream",
            )
        })?;
    let ResolvedSource::LocalFile { path } = resolved else {
        return Err(NexoraApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "source type cannot be streamed yet",
            "/nexora/trackstream",
        ));
    };
    let format = AudioFormat::new(48_000, 2, nexora_audio_types::AudioCodec::Opus);
    let output = std::env::temp_dir().join(format!(
        "nexora-trackstream-{}.opus",
        nexora_common::TimestampMs::now().as_millis()
    ));
    let input = path.clone();
    let output_path = output.clone();
    tokio::task::spawn_blocking(move || {
        nexora_audio_pipeline::process_file(
            &input,
            &output_path,
            format.sample_rate,
            format.channels,
            nexora_audio_pipeline::OutputCodec::Opus,
        )
    })
    .await
    .map_err(|_| {
        NexoraApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stream task failed",
            "/nexora/trackstream",
        )
    })?
    .map_err(|error| {
        NexoraApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
            "/nexora/trackstream",
        )
    })?;
    let bytes = tokio::fs::read(&output).await.map_err(|_| {
        NexoraApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stream output is unreadable",
            "/nexora/trackstream",
        )
    })?;
    let _ = tokio::fs::remove_file(&output).await;
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "audio/ogg")],
        axum::body::Body::from(bytes),
    )
        .into_response())
}
