use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use nexora_audio_pipeline::OpusFrame;
use nexora_audio_types::AudioFormat;
use nexora_common::{ApiVersion, SessionId, TimestampMs, TrackId};
use nexora_error::{NexoraError, NexoraResult};
use nexora_protocol::{
    CreateSessionRequest, CreateSessionResponse, EventEnvelope, PlayRequest, SessionEventType,
    SessionState,
};
use serde_json::json;
use tokio::sync::{RwLock, broadcast, watch};
use tokio::task::JoinHandle;

pub mod nexora;

pub use nexora::{
    NexoraPlayerRecord, NexoraSessionContext, NexoraSessionManager, PLAYER_UPDATE_INTERVAL_SECONDS,
};

/// Signals sent from session state to the single active playback task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackSignal {
    Pause,
    Resume,
    Stop,
    Seek(u64),
    SetVolume(u8),
}

/// Per-session broadcast channel for Opus frames produced by the pacing task.
#[derive(Debug, Default)]
pub struct PlaybackRegistry {
    sessions: RwLock<HashMap<SessionId, broadcast::Sender<OpusFrame>>>,
}

impl PlaybackRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a broadcast sender for a session. Returns `true` when this is
    /// the first sender registered (the pacing task should start producing).
    pub async fn register(
        &self,
        session_id: &SessionId,
        sender: broadcast::Sender<OpusFrame>,
    ) -> bool {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), sender).is_none()
    }

    /// Remove the broadcast sender for a session (playback ended or session
    /// stopped).
    pub async fn deregister(&self, session_id: &SessionId) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    /// Send one Opus frame to all subscribers of a session's audio channel.
    pub async fn send_frame(&self, session_id: &SessionId, frame: OpusFrame) -> bool {
        let sessions = self.sessions.read().await;
        match sessions.get(session_id) {
            Some(sender) => sender.send(frame).is_ok(),
            None => false,
        }
    }

    /// Returns `true` when a session currently has an audio channel registered.
    pub async fn has_channel(&self, session_id: &SessionId) -> bool {
        let sessions = self.sessions.read().await;
        sessions.contains_key(session_id)
    }

    /// Subscribe to Opus frames for a session. Returns `None` when no audio
    /// channel is registered for the session.
    pub async fn subscribe(
        &self,
        session_id: &SessionId,
    ) -> Option<broadcast::Receiver<OpusFrame>> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|sender| sender.subscribe())
    }
}

#[derive(Debug, Clone)]
struct SessionRecord {
    status: String,
    current_track: Option<TrackId>,
    position_ms: u64,
    volume: u8,
    queue: Vec<TrackId>,
    output: AudioFormat,
    playback: Option<watch::Sender<PlaybackSignal>>,
    audio_out: Option<broadcast::Sender<OpusFrame>>,
    /// Handle to the active playback task, so a replacement task can abort it.
    playback_task: Option<Arc<JoinHandle<()>>>,
}

#[derive(Debug)]
pub struct SessionManager {
    sessions: RwLock<HashMap<SessionId, SessionRecord>>,
    audio: PlaybackRegistry,
    events: broadcast::Sender<EventEnvelope>,
    counter: AtomicU64,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        let (events, _receiver) = broadcast::channel(256);
        Self {
            sessions: RwLock::new(HashMap::new()),
            audio: PlaybackRegistry::new(),
            events,
            counter: AtomicU64::new(0),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.events.subscribe()
    }

    /// Returns the per-session Opus frame broadcast registry.
    pub fn audio_registry(&self) -> &PlaybackRegistry {
        &self.audio
    }

    /// Attach the one playback task allowed to own this session.
    pub async fn attach_playback(
        &self,
        session_id: &SessionId,
        sender: watch::Sender<PlaybackSignal>,
        audio_out: broadcast::Sender<OpusFrame>,
    ) -> NexoraResult<()> {
        let mut sessions = self.sessions.write().await;
        let record = sessions
            .get_mut(session_id)
            .ok_or_else(|| NexoraError::service("session not found"))?;
        record.playback = Some(sender);
        record.audio_out = Some(audio_out);
        Ok(())
    }

    /// Remove a playback task that has ended or been replaced.
    pub async fn detach_playback(&self, session_id: &SessionId) {
        let mut sessions = self.sessions.write().await;
        if let Some(record) = sessions.get_mut(session_id) {
            record.playback = None;
            record.audio_out = None;
            if let Some(handle) = record.playback_task.take() {
                handle.abort();
            }
        }
    }

    /// Abort the active playback task without touching session state.
    ///
    /// Called by `start_playback` before spawning a replacement task for the
    // same session, so a replaced track never runs two playback tasks at once.
    pub async fn abort_playback_task(&self, session_id: &SessionId) {
        let mut sessions = self.sessions.write().await;
        let abort = sessions
            .get_mut(session_id)
            .and_then(|record| record.playback_task.take());
        if let Some(handle) = abort {
            handle.abort();
        }
    }

    /// Store the playback task handle so a replacement task can abort it.
    pub async fn store_playback_task(&self, session_id: &SessionId, handle: Arc<JoinHandle<()>>) {
        let mut sessions = self.sessions.write().await;
        if let Some(record) = sessions.get_mut(session_id) {
            record.playback_task = Some(handle);
        }
    }

    fn signal_playback(&self, session_id: &SessionId, signal: PlaybackSignal) {
        let sessions = self.sessions.try_read();
        if let Ok(sessions) = sessions
            && let Some(record) = sessions.get(session_id)
            && let Some(sender) = &record.playback
        {
            let _ = sender.send(signal);
        }
    }

    /// Advance the authoritative position from the playback task.
    pub async fn update_position(
        &self,
        session_id: &SessionId,
        position_ms: u64,
    ) -> NexoraResult<()> {
        let mut sessions = self.sessions.write().await;
        let record = sessions
            .get_mut(session_id)
            .ok_or_else(|| NexoraError::service("session not found"))?;
        record.position_ms = position_ms;
        Ok(())
    }

    /// Mark the current track as finished and clear it from the session.
    /// Queue auto-advance belongs to the playback task, not session state.
    pub async fn finish(&self, session_id: &SessionId) -> NexoraResult<()> {
        {
            let mut sessions = self.sessions.write().await;
            let record = sessions
                .get_mut(session_id)
                .ok_or_else(|| NexoraError::service("session not found"))?;
            record.status = "stopped".to_string();
            record.current_track = None;
            record.position_ms = 0;
            record.playback = None;
        }
        self.emit(
            session_id,
            SessionEventType::TrackEnded,
            json!({ "reason": "finished" }),
        );
        Ok(())
    }

    /// Mark the current track as ended while the playback task keeps running.
    ///
    /// The playback task owns queue advancement: it clears the finished track
    /// here and then starts the next queued one.
    pub async fn end_track(&self, session_id: &SessionId) -> NexoraResult<()> {
        {
            let mut sessions = self.sessions.write().await;
            let record = sessions
                .get_mut(session_id)
                .ok_or_else(|| NexoraError::service("session not found"))?;
            record.current_track = None;
            record.position_ms = 0;
        }
        self.emit(
            session_id,
            SessionEventType::TrackEnded,
            json!({ "reason": "finished" }),
        );
        Ok(())
    }

    /// Mark the current track as failed and clear it from the session.
    pub async fn fail(&self, session_id: &SessionId, reason: &str) -> NexoraResult<()> {
        {
            let mut sessions = self.sessions.write().await;
            let record = sessions
                .get_mut(session_id)
                .ok_or_else(|| NexoraError::service("session not found"))?;
            record.status = "stopped".to_string();
            record.current_track = None;
            record.position_ms = 0;
            record.playback = None;
        }
        self.emit(
            session_id,
            SessionEventType::TrackFailed,
            json!({ "reason": reason }),
        );
        Ok(())
    }

    pub async fn create(&self, output: AudioFormat) -> NexoraResult<CreateSessionResponse> {
        let sequence = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let session_id = SessionId::new(format!("nx_session_{sequence}"))?;
        self.create_with_id(session_id, output).await
    }

    /// Creates a session under a caller-chosen id.
    ///
    /// The Nexora nexora surface hands out its own sixteen-character session
    /// ids, and those ids must address the same session on every route.
    pub async fn create_with_id(
        &self,
        session_id: SessionId,
        output: AudioFormat,
    ) -> NexoraResult<CreateSessionResponse> {
        let record = SessionRecord {
            status: "created".to_string(),
            current_track: None,
            position_ms: 0,
            volume: 100,
            queue: Vec::new(),
            output,
            playback: None,
            audio_out: None,
            playback_task: None,
        };
        self.sessions
            .write()
            .await
            .insert(session_id.clone(), record);
        self.emit(&session_id, SessionEventType::SessionCreated, json!({}));
        Ok(CreateSessionResponse {
            session_id,
            status: "created".to_string(),
            engine: "nexora".to_string(),
            api_version: ApiVersion::V1,
        })
    }

    pub async fn create_from_request(
        &self,
        request: CreateSessionRequest,
    ) -> NexoraResult<CreateSessionResponse> {
        self.create(request.output).await
    }

    pub async fn get(&self, session_id: &SessionId) -> NexoraResult<SessionState> {
        let sessions = self.sessions.read().await;
        let record = sessions
            .get(session_id)
            .ok_or_else(|| NexoraError::service("session not found"))?;
        Ok(SessionState {
            session_id: session_id.clone(),
            status: record.status.clone(),
            position_ms: record.position_ms,
            volume: record.volume,
        })
    }

    pub async fn play(&self, session_id: &SessionId, request: PlayRequest) -> NexoraResult<()> {
        {
            let mut sessions = self.sessions.write().await;
            let record = sessions
                .get_mut(session_id)
                .ok_or_else(|| NexoraError::service("session not found"))?;
            record.current_track = Some(request.track_id.clone());
            record.position_ms = request.start_position_ms;
            record.status = "playing".to_string();
        }
        self.emit(
            session_id,
            SessionEventType::TrackStarted,
            json!({ "track_id": request.track_id.as_str() }),
        );
        Ok(())
    }

    pub async fn pause(&self, session_id: &SessionId) -> NexoraResult<()> {
        self.set_status(session_id, "paused", SessionEventType::PlaybackPaused)
            .await?;
        self.signal_playback(session_id, PlaybackSignal::Pause);
        Ok(())
    }

    pub async fn resume(&self, session_id: &SessionId) -> NexoraResult<()> {
        self.set_status(session_id, "playing", SessionEventType::PlaybackResumed)
            .await?;
        self.signal_playback(session_id, PlaybackSignal::Resume);
        Ok(())
    }

    async fn set_status(
        &self,
        session_id: &SessionId,
        status: &str,
        event: SessionEventType,
    ) -> NexoraResult<()> {
        {
            let mut sessions = self.sessions.write().await;
            let record = sessions
                .get_mut(session_id)
                .ok_or_else(|| NexoraError::service("session not found"))?;
            record.status = status.to_string();
        }
        self.emit(session_id, event, json!({}));
        Ok(())
    }

    pub async fn stop(&self, session_id: &SessionId) -> NexoraResult<()> {
        {
            let mut sessions = self.sessions.write().await;
            let record = sessions
                .get_mut(session_id)
                .ok_or_else(|| NexoraError::service("session not found"))?;
            record.status = "stopped".to_string();
            record.current_track = None;
            record.position_ms = 0;
            if let Some(handle) = record.playback_task.take() {
                handle.abort();
            }
        }
        self.signal_playback(session_id, PlaybackSignal::Stop);
        self.emit(session_id, SessionEventType::PlaybackStopped, json!({}));
        Ok(())
    }

    pub async fn seek(&self, session_id: &SessionId, position_ms: u64) -> NexoraResult<()> {
        {
            let mut sessions = self.sessions.write().await;
            let record = sessions
                .get_mut(session_id)
                .ok_or_else(|| NexoraError::service("session not found"))?;
            record.position_ms = position_ms;
        }
        self.signal_playback(session_id, PlaybackSignal::Seek(position_ms));
        self.emit(
            session_id,
            SessionEventType::TrackPosition,
            json!({ "position_ms": position_ms }),
        );
        Ok(())
    }

    pub async fn set_volume(&self, session_id: &SessionId, volume: u8) -> NexoraResult<()> {
        if volume > 150 {
            return Err(NexoraError::invalid_input("volume out of range"));
        }
        {
            let mut sessions = self.sessions.write().await;
            let record = sessions
                .get_mut(session_id)
                .ok_or_else(|| NexoraError::service("session not found"))?;
            record.volume = volume;
        }
        self.signal_playback(session_id, PlaybackSignal::SetVolume(volume));
        self.emit(
            session_id,
            SessionEventType::SessionReady,
            json!({ "volume": volume }),
        );
        Ok(())
    }

    pub async fn enqueue(&self, session_id: &SessionId, track_id: TrackId) -> NexoraResult<usize> {
        let len = {
            let mut sessions = self.sessions.write().await;
            let record = sessions
                .get_mut(session_id)
                .ok_or_else(|| NexoraError::service("session not found"))?;
            record.queue.push(track_id);
            record.queue.len()
        };
        self.emit(
            session_id,
            SessionEventType::QueueUpdated,
            json!({ "queue_length": len }),
        );
        Ok(len)
    }

    pub async fn queue(&self, session_id: &SessionId) -> NexoraResult<Vec<TrackId>> {
        let sessions = self.sessions.read().await;
        let record = sessions
            .get(session_id)
            .ok_or_else(|| NexoraError::service("session not found"))?;
        Ok(record.queue.clone())
    }

    /// Pop the next queued track from the front (FIFO), returning `None` if the
    /// queue is empty.
    pub async fn pop_next_queued(&self, session_id: &SessionId) -> NexoraResult<Option<TrackId>> {
        let mut sessions = self.sessions.write().await;
        let record = sessions
            .get_mut(session_id)
            .ok_or_else(|| NexoraError::service("session not found"))?;
        Ok(if record.queue.is_empty() {
            None
        } else {
            Some(record.queue.remove(0))
        })
    }

    pub async fn current_track(&self, session_id: &SessionId) -> NexoraResult<Option<TrackId>> {
        let sessions = self.sessions.read().await;
        let record = sessions
            .get(session_id)
            .ok_or_else(|| NexoraError::service("session not found"))?;
        Ok(record.current_track.clone())
    }

    pub async fn output_format(&self, session_id: &SessionId) -> NexoraResult<AudioFormat> {
        let sessions = self.sessions.read().await;
        let record = sessions
            .get(session_id)
            .ok_or_else(|| NexoraError::service("session not found"))?;
        Ok(record.output.clone())
    }

    fn emit(
        &self,
        session_id: &SessionId,
        event_type: SessionEventType,
        payload: serde_json::Value,
    ) {
        let envelope = EventEnvelope {
            event_id: format!("evt_{}", self.counter.load(Ordering::SeqCst)),
            version: ApiVersion::V1,
            timestamp: TimestampMs::now(),
            session_id: session_id.clone(),
            event_type,
            payload,
            correlation_id: "api-v1".to_string(),
        };
        let _ = self.events.send(envelope);
    }
}
