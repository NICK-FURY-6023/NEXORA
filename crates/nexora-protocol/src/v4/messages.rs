//! The Lavalink v4 WebSocket frames.
//!
//! The reference emits every event **flat**: the `op`, the `type`
//! discriminator, the `guildId` and the event's own fields are siblings in one
//! object, not nested under a `data` wrapper (bytecode evidence on
//! `Message$EmittedEvent` in `protocol-jvm-4.2.2.jar`).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::error::V4Exception;
use super::op::V4Op;
use super::player::V4PlayerState;
use super::track::V4Track;

/// Why a track stopped playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum V4TrackEndReason {
    #[serde(rename = "finished")]
    Finished,
    #[serde(rename = "loadFailed")]
    LoadFailed,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "replaced")]
    Replaced,
    #[serde(rename = "cleanup")]
    Cleanup,
}

impl V4TrackEndReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            V4TrackEndReason::Finished => "finished",
            V4TrackEndReason::LoadFailed => "loadFailed",
            V4TrackEndReason::Stopped => "stopped",
            V4TrackEndReason::Replaced => "replaced",
            V4TrackEndReason::Cleanup => "cleanup",
        }
    }

    /// Whether the reference expects the client to advance its own queue.
    pub fn may_start_next(&self) -> bool {
        matches!(
            self,
            V4TrackEndReason::Finished | V4TrackEndReason::LoadFailed
        )
    }
}

/// The `ready` frame: `{"op":"ready","resumed":…,"sessionId":"…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4Ready {
    pub resumed: bool,
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

impl V4Ready {
    /// The frame as the reference sends it.
    pub fn to_frame(&self) -> Value {
        let mut frame = serde_json::to_value(self).unwrap_or(Value::Null);
        insert_op(&mut frame, V4Op::Ready);
        frame
    }
}

/// The `playerUpdate` frame: `{"op":"playerUpdate","state":…,"guildId":"…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4PlayerUpdateFrame {
    pub state: V4PlayerState,
    #[serde(rename = "guildId")]
    pub guild_id: String,
}

impl V4PlayerUpdateFrame {
    /// The frame as the reference sends it.
    pub fn to_frame(&self) -> Value {
        let mut frame = serde_json::to_value(self).unwrap_or(Value::Null);
        insert_op(&mut frame, V4Op::PlayerUpdate);
        frame
    }
}

/// The `stats` frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct V4StatsFrame {
    #[serde(rename = "frameStats")]
    pub frame_stats: Option<Value>,
    pub players: i64,
    #[serde(rename = "playingPlayers")]
    pub playing_players: i64,
    pub uptime: i64,
    pub memory: Value,
    pub cpu: Value,
}

impl V4StatsFrame {
    /// The frame as the reference sends it.
    pub fn to_frame(&self) -> Value {
        let mut frame = serde_json::to_value(self).unwrap_or(Value::Null);
        insert_op(&mut frame, V4Op::Stats);
        frame
    }
}

/// One emitted event, discriminated by its `type` field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum V4EventMessage {
    #[serde(rename = "TrackStartEvent")]
    TrackStart {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: V4Track,
    },
    #[serde(rename = "TrackEndEvent")]
    TrackEnd {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: V4Track,
        reason: V4TrackEndReason,
    },
    #[serde(rename = "TrackExceptionEvent")]
    TrackException {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: V4Track,
        exception: V4Exception,
    },
    #[serde(rename = "TrackStuckEvent")]
    TrackStuck {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: V4Track,
        #[serde(rename = "thresholdMs")]
        threshold_ms: u64,
    },
    #[serde(rename = "WebSocketClosedEvent")]
    WebSocketClosed {
        #[serde(rename = "guildId")]
        guild_id: String,
        code: i64,
        reason: String,
        #[serde(rename = "byRemote")]
        by_remote: bool,
    },
}

impl V4EventMessage {
    /// The `type` discriminator of this event.
    pub fn type_name(&self) -> &'static str {
        match self {
            V4EventMessage::TrackStart { .. } => "TrackStartEvent",
            V4EventMessage::TrackEnd { .. } => "TrackEndEvent",
            V4EventMessage::TrackException { .. } => "TrackExceptionEvent",
            V4EventMessage::TrackStuck { .. } => "TrackStuckEvent",
            V4EventMessage::WebSocketClosed { .. } => "WebSocketClosedEvent",
        }
    }

    /// The frame as the reference sends it: `op`, `type` and the event's own
    /// fields are siblings.
    pub fn to_frame(&self) -> Value {
        let mut frame = serde_json::to_value(self).unwrap_or(Value::Null);
        insert_op(&mut frame, V4Op::Event);
        frame
    }
}

/// Inserts the `op` field into an already-serialised frame.
fn insert_op(frame: &mut Value, op: V4Op) {
    if let Some(target) = frame.as_object_mut() {
        target.insert("op".to_owned(), json!(op.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v4::error::V4ErrorSeverity;
    use crate::v4::track::V4TrackInfo;

    fn sample_track() -> V4Track {
        V4Track {
            encoded: "abc".into(),
            info: V4TrackInfo {
                identifier: "tone.wav".into(),
                is_seekable: true,
                author: "artist".into(),
                length: 100,
                is_stream: false,
                position: 0,
                title: "Tone".into(),
                uri: None,
                artwork_url: None,
                isrc: None,
                source_name: "local".into(),
            },
            plugin_info: json!({}),
            user_data: json!({}),
        }
    }

    #[test]
    fn the_ready_frame_matches_the_reference_shape() {
        let frame = V4Ready {
            resumed: false,
            session_id: "abc".into(),
        }
        .to_frame();
        assert_eq!(frame["op"], "ready");
        assert_eq!(frame["resumed"], false);
        assert_eq!(frame["sessionId"], "abc");
    }

    #[test]
    fn the_player_update_frame_matches_the_reference_shape() {
        let frame = V4PlayerUpdateFrame {
            state: V4PlayerState::default(),
            guild_id: "1234".into(),
        }
        .to_frame();
        assert_eq!(frame["op"], "playerUpdate");
        assert_eq!(frame["guildId"], "1234");
        assert_eq!(frame["state"]["ping"], -1);
    }

    #[test]
    fn an_event_frame_is_flat_with_no_data_wrapper() {
        let frame = V4EventMessage::TrackStart {
            guild_id: "1234".into(),
            track: sample_track(),
        }
        .to_frame();
        assert_eq!(frame["op"], "event");
        assert_eq!(frame["type"], "TrackStartEvent");
        assert_eq!(frame["guildId"], "1234");
        assert_eq!(frame["track"]["info"]["title"], "Tone");
        assert!(frame.get("data").is_none());
        assert!(frame.get("event").is_none());
    }

    #[test]
    fn a_track_end_frame_carries_the_reason() {
        let frame = V4EventMessage::TrackEnd {
            guild_id: "1234".into(),
            track: sample_track(),
            reason: V4TrackEndReason::Replaced,
        }
        .to_frame();
        assert_eq!(frame["type"], "TrackEndEvent");
        assert_eq!(frame["reason"], "replaced");
    }

    #[test]
    fn an_exception_frame_carries_the_exception() {
        let frame = V4EventMessage::TrackException {
            guild_id: "1234".into(),
            track: sample_track(),
            exception: V4Exception {
                message: "boom".into(),
                severity: V4ErrorSeverity::Suspicious,
                cause: None,
                cause_stack_trace: None,
            },
        }
        .to_frame();
        assert_eq!(frame["type"], "TrackExceptionEvent");
        assert_eq!(frame["exception"]["severity"], "suspicious");
    }

    #[test]
    fn end_reasons_use_the_reference_names() {
        assert_eq!(V4TrackEndReason::Finished.as_str(), "finished");
        assert_eq!(V4TrackEndReason::LoadFailed.as_str(), "loadFailed");
        assert_eq!(V4TrackEndReason::Stopped.as_str(), "stopped");
        assert_eq!(V4TrackEndReason::Replaced.as_str(), "replaced");
        assert_eq!(V4TrackEndReason::Cleanup.as_str(), "cleanup");
        assert!(V4TrackEndReason::Finished.may_start_next());
        assert!(!V4TrackEndReason::Stopped.may_start_next());
    }
}
