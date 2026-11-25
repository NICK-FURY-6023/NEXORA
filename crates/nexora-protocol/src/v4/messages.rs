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
    /// NodeLink's gapless transition: the next track starts without a gap.
    #[serde(rename = "gapless")]
    Gapless,
    /// NodeLink's crossfade transition.
    #[serde(rename = "crossfading")]
    Crossfading,
}

impl V4TrackEndReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            V4TrackEndReason::Finished => "finished",
            V4TrackEndReason::LoadFailed => "loadFailed",
            V4TrackEndReason::Stopped => "stopped",
            V4TrackEndReason::Replaced => "replaced",
            V4TrackEndReason::Cleanup => "cleanup",
            V4TrackEndReason::Gapless => "gapless",
            V4TrackEndReason::Crossfading => "crossfading",
        }
    }

    /// Whether the reference expects the client to advance its own queue.
    pub fn may_start_next(&self) -> bool {
        matches!(
            self,
            V4TrackEndReason::Finished
                | V4TrackEndReason::LoadFailed
                | V4TrackEndReason::Gapless
                | V4TrackEndReason::Crossfading
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// One frame the v4 socket can send.
///
/// The reference's `Message` interface is the supertype of both the
/// `playerUpdate` frame and the emitted events, and the socket serialises
/// either through the same polymorphic serializer
/// (bytecode evidence on `Message$Serializer` in `protocol-jvm-4.2.2.jar`).
/// This enum is that supertype.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum V4Frame {
    /// A `playerUpdate` frame: its own op, not an `event`.
    PlayerUpdate(Box<V4PlayerUpdateFrame>),
    /// An emitted event: `op:"event"` with a `type` discriminator.
    Event(Box<V4EventMessage>),
}

impl V4Frame {
    /// The frame as the reference sends it.
    pub fn to_frame(&self) -> Value {
        match self {
            V4Frame::PlayerUpdate(frame) => frame.to_frame(),
            V4Frame::Event(event) => event.to_frame(),
        }
    }
}

/// One emitted event, discriminated by its `type` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum V4EventMessage {
    #[serde(rename = "TrackStartEvent")]
    TrackStart {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: Box<V4Track>,
    },
    #[serde(rename = "TrackEndEvent")]
    TrackEnd {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: Box<V4Track>,
        reason: V4TrackEndReason,
    },
    #[serde(rename = "TrackExceptionEvent")]
    TrackException {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: Box<V4Track>,
        exception: V4Exception,
    },
    #[serde(rename = "TrackStuckEvent")]
    TrackStuck {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: Box<V4Track>,
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
    /// NodeLink's seek event: `{position, duration}`.
    #[serde(rename = "SeekEvent")]
    Seek {
        #[serde(rename = "guildId")]
        guild_id: String,
        position: u64,
        duration: i64,
    },
    /// NodeLink's pause event: `{paused}`.
    #[serde(rename = "PauseEvent")]
    Pause {
        #[serde(rename = "guildId")]
        guild_id: String,
        paused: bool,
    },
    /// NodeLink's volume event: `{volume}`.
    #[serde(rename = "VolumeChangedEvent")]
    VolumeChanged {
        #[serde(rename = "guildId")]
        guild_id: String,
        volume: i64,
    },
    /// NodeLink's filter event: `{filters}`.
    #[serde(rename = "FiltersChangedEvent")]
    FiltersChanged {
        #[serde(rename = "guildId")]
        guild_id: String,
        filters: super::filters::V4Filters,
    },
    /// NodeLink's player-created event: `{guildId, player}`.
    #[serde(rename = "PlayerCreatedEvent")]
    PlayerCreated {
        #[serde(rename = "guildId")]
        guild_id: String,
        player: Box<super::player::V4Player>,
    },
    /// NodeLink's player-destroyed event: `{guildId}`.
    #[serde(rename = "PlayerDestroyedEvent")]
    PlayerDestroyed {
        #[serde(rename = "guildId")]
        guild_id: String,
    },
    /// Discord's voice state update: `{guildId, userId, channelId, selfDeaf, selfMute}`.
    #[serde(rename = "VoiceStateUpdate")]
    VoiceStateUpdate {
        #[serde(rename = "guildId")]
        guild_id: String,
        #[serde(rename = "userId")]
        user_id: String,
        #[serde(rename = "channelId")]
        channel_id: Option<String>,
        #[serde(rename = "selfDeaf")]
        self_deaf: bool,
        #[serde(rename = "selfMute")]
        self_mute: bool,
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
            V4EventMessage::Seek { .. } => "SeekEvent",
            V4EventMessage::Pause { .. } => "PauseEvent",
            V4EventMessage::VolumeChanged { .. } => "VolumeChangedEvent",
            V4EventMessage::FiltersChanged { .. } => "FiltersChangedEvent",
            V4EventMessage::PlayerCreated { .. } => "PlayerCreatedEvent",
            V4EventMessage::PlayerDestroyed { .. } => "PlayerDestroyedEvent",
            V4EventMessage::VoiceStateUpdate { .. } => "VoiceStateUpdate",
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
                filters: None,
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
            track: Box::new(sample_track()),
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
    fn a_voice_state_update_frame_matches_the_reference_shape() {
        let frame = V4EventMessage::VoiceStateUpdate {
            guild_id: "1234".into(),
            user_id: "4321".into(),
            channel_id: Some("5678".into()),
            self_deaf: true,
            self_mute: false,
        }
        .to_frame();
        assert_eq!(frame["op"], "event");
        assert_eq!(frame["type"], "VoiceStateUpdate");
        assert_eq!(frame["guildId"], "1234");
        assert_eq!(frame["userId"], "4321");
        assert_eq!(frame["channelId"], "5678");
        assert_eq!(frame["selfDeaf"], true);
        assert_eq!(frame["selfMute"], false);
        assert!(frame.get("data").is_none());
    }

    #[test]
    fn a_voice_state_update_without_a_channel_sends_null_channel_id() {
        let frame = V4EventMessage::VoiceStateUpdate {
            guild_id: "1234".into(),
            user_id: "4321".into(),
            channel_id: None,
            self_deaf: false,
            self_mute: true,
        }
        .to_frame();
        assert_eq!(frame["channelId"], Value::Null);
    }

    #[test]
    fn a_track_end_frame_carries_the_reason() {
        let frame = V4EventMessage::TrackEnd {
            guild_id: "1234".into(),
            track: Box::new(sample_track()),
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
            track: Box::new(sample_track()),
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

    #[test]
    fn the_nodelink_reasons_use_their_own_names() {
        assert_eq!(V4TrackEndReason::Gapless.as_str(), "gapless");
        assert_eq!(V4TrackEndReason::Crossfading.as_str(), "crossfading");
        // A transition to the next track is a reason to advance the queue.
        assert!(V4TrackEndReason::Gapless.may_start_next());
        assert!(V4TrackEndReason::Crossfading.may_start_next());
    }

    #[test]
    fn the_nodelink_events_carry_their_payloads() {
        let frame = V4EventMessage::Seek {
            guild_id: "1234".into(),
            position: 5_000,
            duration: -1_000,
        }
        .to_frame();
        assert_eq!(frame["op"], "event");
        assert_eq!(frame["type"], "SeekEvent");
        assert_eq!(frame["guildId"], "1234");
        assert_eq!(frame["position"], 5_000);
        assert_eq!(frame["duration"], -1_000);

        let frame = V4EventMessage::Pause {
            guild_id: "1234".into(),
            paused: true,
        }
        .to_frame();
        assert_eq!(frame["type"], "PauseEvent");
        assert_eq!(frame["paused"], true);

        let frame = V4EventMessage::VolumeChanged {
            guild_id: "1234".into(),
            volume: 80,
        }
        .to_frame();
        assert_eq!(frame["type"], "VolumeChangedEvent");
        assert_eq!(frame["volume"], 80);

        let frame = V4EventMessage::FiltersChanged {
            guild_id: "1234".into(),
            filters: crate::v4::filters::V4Filters::default(),
        }
        .to_frame();
        assert_eq!(frame["type"], "FiltersChangedEvent");
        assert!(frame["filters"].is_object());

        let frame = V4EventMessage::PlayerDestroyed {
            guild_id: "1234".into(),
        }
        .to_frame();
        assert_eq!(frame["type"], "PlayerDestroyedEvent");
        assert_eq!(frame["guildId"], "1234");
    }
}
