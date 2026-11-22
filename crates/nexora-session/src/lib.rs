use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use nexora_audio_types::AudioFormat;
use nexora_common::{ApiVersion, SessionId, TimestampMs, TrackId};
use nexora_error::{NexoraError, NexoraResult};
use nexora_protocol::{
    CreateSessionRequest, CreateSessionResponse, EventEnvelope, PlayRequest, SessionEventType,
    SessionState,
};
use serde_json::json;
use tokio::sync::{RwLock, broadcast};

#[derive(Debug, Clone)]
struct SessionRecord {
    status: String,
    current_track: Option<TrackId>,
    position_ms: u64,
    volume: u8,
    queue: Vec<TrackId>,
    output: AudioFormat,
}

#[derive(Debug)]
pub struct SessionManager {
    sessions: RwLock<HashMap<SessionId, SessionRecord>>,
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
            events,
            counter: AtomicU64::new(0),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.events.subscribe()
    }

    pub async fn create(&self, output: AudioFormat) -> NexoraResult<CreateSessionResponse> {
        let sequence = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let session_id = SessionId::new(format!("nx_session_{sequence}"))?;
        let record = SessionRecord {
            status: "created".to_string(),
            current_track: None,
            position_ms: 0,
            volume: 100,
            queue: Vec::new(),
            output,
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
            .await
    }

    pub async fn resume(&self, session_id: &SessionId) -> NexoraResult<()> {
        self.set_status(session_id, "playing", SessionEventType::PlaybackResumed)
            .await
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
        }
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
