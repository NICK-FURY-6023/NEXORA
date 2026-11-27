//! Restart-safe snapshots of in-memory session state.
//!
//! A snapshot records only durable state: status, position, volume, queue,
//! track and output format. Live playback handles, broadcast channels and
//! event subscriptions are deliberately not saved; a restored session starts
//! with no playback task attached.

use std::path::Path;

use nexora_audio_types::AudioFormat;
use nexora_common::{SessionId, TrackId};
use nexora_error::{NexoraError, NexoraResult};
use serde::{Deserialize, Serialize};

use crate::{SessionManager, SessionRecord};

/// One session's durable state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshotEntry {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub status: String,
    #[serde(rename = "currentTrack")]
    pub current_track: Option<String>,
    #[serde(rename = "positionMs")]
    pub position_ms: u64,
    pub volume: u8,
    pub queue: Vec<String>,
    pub output: AudioFormat,
}

/// A restart-safe view of every session that is still live.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub sessions: Vec<SessionSnapshotEntry>,
}

/// Captures durable state from a running manager.
pub async fn snapshot(manager: &SessionManager) -> SessionSnapshot {
    let sessions = manager.sessions.read().await;
    let sessions = sessions
        .iter()
        .map(|(id, record)| SessionSnapshotEntry {
            session_id: id.as_str().to_owned(),
            status: record.status.clone(),
            current_track: record
                .current_track
                .as_ref()
                .map(|id| id.as_str().to_owned()),
            position_ms: record.position_ms,
            volume: record.volume,
            queue: record
                .queue
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect(),
            output: record.output.clone(),
        })
        .collect();
    SessionSnapshot { sessions }
}

/// Rebuilds a manager from a snapshot.
///
/// A session that was explicitly stopped is not restored, and no playback
/// task is attached: a restart resumes state, never audio.
pub fn restore_snapshot(snapshot: SessionSnapshot) -> NexoraResult<SessionManager> {
    let restored = SessionManager::new();
    for entry in snapshot.sessions {
        if entry.status == "stopped" {
            continue;
        }
        let session_id = SessionId::new(entry.session_id.clone())
            .map_err(|_| NexoraError::invalid_input("snapshot contains an invalid session id"))?;
        let current_track = entry
            .current_track
            .map(|id| TrackId::new(id).map_err(|_| NexoraError::invalid_input("invalid track id")))
            .transpose()?;
        let queue = entry
            .queue
            .into_iter()
            .map(|id| TrackId::new(id).map_err(|_| NexoraError::invalid_input("invalid queue id")))
            .collect::<NexoraResult<Vec<_>>>()?;
        let record = SessionRecord {
            status: entry.status,
            current_track,
            position_ms: entry.position_ms,
            volume: entry.volume,
            queue,
            output: entry.output,
            playback: None,
            audio_out: None,
            playback_task: None,
        };
        restored
            .sessions
            .try_write()
            .map_err(|_| NexoraError::service("session state is busy"))?
            .insert(session_id, record);
    }
    Ok(restored)
}

/// Writes a snapshot as JSON under the caller's path.
pub async fn save_to_file(manager: &SessionManager, path: &Path) -> NexoraResult<()> {
    let bytes = serde_json::to_vec_pretty(&snapshot(manager).await)
        .map_err(|_| NexoraError::service("session snapshot cannot be encoded"))?;
    tokio::fs::write(path, bytes)
        .await
        .map_err(|_| NexoraError::service("session snapshot cannot be written"))
}

/// Reads a snapshot written by [`save_to_file`].
pub async fn load_from_file(path: &Path) -> NexoraResult<SessionManager> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| NexoraError::service("session snapshot cannot be read"))?;
    let snapshot: SessionSnapshot = serde_json::from_slice(&bytes)
        .map_err(|_| NexoraError::invalid_input("session snapshot is not valid JSON"))?;
    restore_snapshot(snapshot)
}
