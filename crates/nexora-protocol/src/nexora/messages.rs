//! The Nexora nexora WebSocket frames.
//!
//! The reference emits every event **flat**: the `op`, the `type`
//! discriminator, the `guildId` and the event's own fields are siblings in one
//! object, not nested under a `data` wrapper (bytecode evidence on
//! `Message$EmittedEvent` in `protocol-jvm-4.2.2.jar`).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::error::NexoraException;
use super::op::NexoraOp;
use super::player::NexoraPlayerState;
use super::track::NexoraTrack;

/// Why a track stopped playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NexoraTrackEndReason {
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
    /// Nexora's gapless transition: the next track starts without a gap.
    #[serde(rename = "gapless")]
    Gapless,
    /// Nexora's crossfade transition.
    #[serde(rename = "crossfading")]
    Crossfading,
}

impl NexoraTrackEndReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            NexoraTrackEndReason::Finished => "finished",
            NexoraTrackEndReason::LoadFailed => "loadFailed",
            NexoraTrackEndReason::Stopped => "stopped",
            NexoraTrackEndReason::Replaced => "replaced",
            NexoraTrackEndReason::Cleanup => "cleanup",
            NexoraTrackEndReason::Gapless => "gapless",
            NexoraTrackEndReason::Crossfading => "crossfading",
        }
    }

    /// Whether the reference expects the client to advance its own queue.
    pub fn may_start_next(&self) -> bool {
        matches!(
            self,
            NexoraTrackEndReason::Finished
                | NexoraTrackEndReason::LoadFailed
                | NexoraTrackEndReason::Gapless
                | NexoraTrackEndReason::Crossfading
        )
    }
}

/// The `ready` frame: `{"op":"ready","resumed":…,"sessionId":"…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraReady {
    pub resumed: bool,
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

impl NexoraReady {
    /// The frame as the reference sends it.
    pub fn to_frame(&self) -> Value {
        let mut frame = serde_json::to_value(self).unwrap_or(Value::Null);
        insert_op(&mut frame, NexoraOp::Ready);
        frame
    }
}

/// The `playerUpdate` frame: `{"op":"playerUpdate","state":…,"guildId":"…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraPlayerUpdateFrame {
    pub state: NexoraPlayerState,
    #[serde(rename = "guildId")]
    pub guild_id: String,
}

impl NexoraPlayerUpdateFrame {
    /// The frame as the reference sends it.
    pub fn to_frame(&self) -> Value {
        let mut frame = serde_json::to_value(self).unwrap_or(Value::Null);
        insert_op(&mut frame, NexoraOp::PlayerUpdate);
        frame
    }
}

/// The `stats` frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexoraStatsFrame {
    #[serde(rename = "frameStats")]
    pub frame_stats: Option<Value>,
    pub players: i64,
    #[serde(rename = "playingPlayers")]
    pub playing_players: i64,
    pub uptime: i64,
    pub memory: Value,
    pub cpu: Value,
}

impl NexoraStatsFrame {
    /// The frame as the reference sends it.
    pub fn to_frame(&self) -> Value {
        let mut frame = serde_json::to_value(self).unwrap_or(Value::Null);
        insert_op(&mut frame, NexoraOp::Stats);
        frame
    }
}

/// One frame the nexora socket can send.
///
/// The reference's `Message` interface is the supertype of both the
/// `playerUpdate` frame and the emitted events, and the socket serialises
/// either through the same polymorphic serializer
/// (bytecode evidence on `Message$Serializer` in `protocol-jvm-4.2.2.jar`).
/// This enum is that supertype.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NexoraFrame {
    /// A `playerUpdate` frame: its own op, not an `event`.
    PlayerUpdate(Box<NexoraPlayerUpdateFrame>),
    /// An emitted event: `op:"event"` with a `type` discriminator.
    Event(Box<NexoraEventMessage>),
}

impl NexoraFrame {
    /// The frame as the reference sends it.
    pub fn to_frame(&self) -> Value {
        match self {
            NexoraFrame::PlayerUpdate(frame) => frame.to_frame(),
            NexoraFrame::Event(event) => event.to_frame(),
        }
    }
}

/// One source's health as it appears on the WebSocket event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceHealth {
    #[serde(rename = "sourceId")]
    pub source_id: String,
    pub healthy: bool,
    pub detail: String,
    #[serde(rename = "checkedAt")]
    pub checked_at: u64,
}

/// One emitted event, discriminated by its `type` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NexoraEventMessage {
    #[serde(rename = "TrackStartEvent")]
    TrackStart {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: Box<NexoraTrack>,
    },
    #[serde(rename = "TrackEndEvent")]
    TrackEnd {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: Box<NexoraTrack>,
        reason: NexoraTrackEndReason,
    },
    #[serde(rename = "TrackExceptionEvent")]
    TrackException {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: Box<NexoraTrack>,
        exception: NexoraException,
    },
    #[serde(rename = "TrackStuckEvent")]
    TrackStuck {
        #[serde(rename = "guildId")]
        guild_id: String,
        track: Box<NexoraTrack>,
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
    /// Nexora's seek event: `{position, duration}`.
    #[serde(rename = "SeekEvent")]
    Seek {
        #[serde(rename = "guildId")]
        guild_id: String,
        position: u64,
        duration: i64,
    },
    /// Nexora's pause event: `{paused}`.
    #[serde(rename = "PauseEvent")]
    Pause {
        #[serde(rename = "guildId")]
        guild_id: String,
        paused: bool,
    },
    /// Nexora's volume event: `{volume}`.
    #[serde(rename = "VolumeChangedEvent")]
    VolumeChanged {
        #[serde(rename = "guildId")]
        guild_id: String,
        volume: i64,
    },
    /// Nexora's filter event: `{filters}`.
    #[serde(rename = "FiltersChangedEvent")]
    FiltersChanged {
        #[serde(rename = "guildId")]
        guild_id: String,
        filters: super::filters::NexoraFilters,
    },
    /// Nexora's player-created event: `{guildId, player}`.
    #[serde(rename = "PlayerCreatedEvent")]
    PlayerCreated {
        #[serde(rename = "guildId")]
        guild_id: String,
        player: Box<super::player::NexoraPlayer>,
    },
    /// Nexora's player-destroyed event: `{guildId}`.
    #[serde(rename = "PlayerDestroyedEvent")]
    PlayerDestroyed {
        #[serde(rename = "guildId")]
        guild_id: String,
    },
    /// Discord's voice state update: `{guildId, userId, channelId, selfDeaf, selfMute}`.
    #[serde(rename = "StreamMetadataEvent")]
    StreamMetadata {
        #[serde(rename = "guildId")]
        guild_id: String,
        #[serde(rename = "trackId")]
        track_id: String,
        title: String,
        #[serde(rename = "sourceName")]
        source_name: String,
        #[serde(rename = "lengthMs")]
        length_ms: u64,
    },
    #[serde(rename = "SourceHealthSnapshotEvent")]
    SourceHealthSnapshot { sources: Vec<NexoraSourceHealth> },
    #[serde(rename = "SourceHealthChangeEvent")]
    SourceHealthChange {
        #[serde(rename = "sourceId")]
        source_id: String,
        healthy: bool,
        detail: String,
    },
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

impl NexoraEventMessage {
    /// The `type` discriminator of this event.
    pub fn type_name(&self) -> &'static str {
        match self {
            NexoraEventMessage::TrackStart { .. } => "TrackStartEvent",
            NexoraEventMessage::TrackEnd { .. } => "TrackEndEvent",
            NexoraEventMessage::TrackException { .. } => "TrackExceptionEvent",
            NexoraEventMessage::TrackStuck { .. } => "TrackStuckEvent",
            NexoraEventMessage::WebSocketClosed { .. } => "WebSocketClosedEvent",
            NexoraEventMessage::Seek { .. } => "SeekEvent",
            NexoraEventMessage::Pause { .. } => "PauseEvent",
            NexoraEventMessage::VolumeChanged { .. } => "VolumeChangedEvent",
            NexoraEventMessage::FiltersChanged { .. } => "FiltersChangedEvent",
            NexoraEventMessage::PlayerCreated { .. } => "PlayerCreatedEvent",
            NexoraEventMessage::PlayerDestroyed { .. } => "PlayerDestroyedEvent",
            NexoraEventMessage::VoiceStateUpdate { .. } => "VoiceStateUpdate",
            NexoraEventMessage::StreamMetadata { .. } => "StreamMetadataEvent",
            NexoraEventMessage::SourceHealthSnapshot { .. } => "SourceHealthSnapshotEvent",
            NexoraEventMessage::SourceHealthChange { .. } => "SourceHealthChangeEvent",
        }
    }

    /// The frame as the reference sends it: `op`, `type` and the event's own
    /// fields are siblings.
    pub fn to_frame(&self) -> Value {
        let mut frame = serde_json::to_value(self).unwrap_or(Value::Null);
        insert_op(&mut frame, NexoraOp::Event);
        frame
    }
}

/// One source's health, as the health events report it.
///
/// The wire type is the protocol's own: the source layer's health struct
/// belongs to that layer, not to the wire contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraSourceHealth {
    #[serde(rename = "sourceId")]
    pub source_id: String,
    pub healthy: bool,
    pub detail: String,
    #[serde(rename = "checkedAt")]
    pub checked_at: u64,
}

/// Inserts the `op` field into an already-serialised frame.
fn insert_op(frame: &mut Value, op: NexoraOp) {
    if let Some(target) = frame.as_object_mut() {
        target.insert("op".to_owned(), json!(op.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexora::error::NexoraErrorSeverity;
    use crate::nexora::track::NexoraTrackInfo;

    fn sample_track() -> NexoraTrack {
        NexoraTrack {
            encoded: "abc".into(),
            info: NexoraTrackInfo {
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
        let frame = NexoraReady {
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
        let frame = NexoraPlayerUpdateFrame {
            state: NexoraPlayerState::default(),
            guild_id: "1234".into(),
        }
        .to_frame();
        assert_eq!(frame["op"], "playerUpdate");
        assert_eq!(frame["guildId"], "1234");
        assert_eq!(frame["state"]["ping"], -1);
    }

    #[test]
    fn an_event_frame_is_flat_with_no_data_wrapper() {
        let frame = NexoraEventMessage::TrackStart {
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
        let frame = NexoraEventMessage::VoiceStateUpdate {
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
        let frame = NexoraEventMessage::VoiceStateUpdate {
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
        let frame = NexoraEventMessage::TrackEnd {
            guild_id: "1234".into(),
            track: Box::new(sample_track()),
            reason: NexoraTrackEndReason::Replaced,
        }
        .to_frame();
        assert_eq!(frame["type"], "TrackEndEvent");
        assert_eq!(frame["reason"], "replaced");
    }

    #[test]
    fn an_exception_frame_carries_the_exception() {
        let frame = NexoraEventMessage::TrackException {
            guild_id: "1234".into(),
            track: Box::new(sample_track()),
            exception: NexoraException {
                message: "boom".into(),
                severity: NexoraErrorSeverity::Suspicious,
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
        assert_eq!(NexoraTrackEndReason::Finished.as_str(), "finished");
        assert_eq!(NexoraTrackEndReason::LoadFailed.as_str(), "loadFailed");
        assert_eq!(NexoraTrackEndReason::Stopped.as_str(), "stopped");
        assert_eq!(NexoraTrackEndReason::Replaced.as_str(), "replaced");
        assert_eq!(NexoraTrackEndReason::Cleanup.as_str(), "cleanup");
        assert!(NexoraTrackEndReason::Finished.may_start_next());
        assert!(!NexoraTrackEndReason::Stopped.may_start_next());
    }

    #[test]
    fn the_nexora_reasons_use_their_own_names() {
        assert_eq!(NexoraTrackEndReason::Gapless.as_str(), "gapless");
        assert_eq!(NexoraTrackEndReason::Crossfading.as_str(), "crossfading");
        // A transition to the next track is a reason to advance the queue.
        assert!(NexoraTrackEndReason::Gapless.may_start_next());
        assert!(NexoraTrackEndReason::Crossfading.may_start_next());
    }

    #[test]
    fn the_nexora_events_carry_their_payloads() {
        let frame = NexoraEventMessage::Seek {
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

        let frame = NexoraEventMessage::Pause {
            guild_id: "1234".into(),
            paused: true,
        }
        .to_frame();
        assert_eq!(frame["type"], "PauseEvent");
        assert_eq!(frame["paused"], true);

        let frame = NexoraEventMessage::VolumeChanged {
            guild_id: "1234".into(),
            volume: 80,
        }
        .to_frame();
        assert_eq!(frame["type"], "VolumeChangedEvent");
        assert_eq!(frame["volume"], 80);

        let frame = NexoraEventMessage::FiltersChanged {
            guild_id: "1234".into(),
            filters: crate::nexora::filters::NexoraFilters::default(),
        }
        .to_frame();
        assert_eq!(frame["type"], "FiltersChangedEvent");
        assert!(frame["filters"].is_object());

        let frame = NexoraEventMessage::PlayerDestroyed {
            guild_id: "1234".into(),
        }
        .to_frame();
        assert_eq!(frame["type"], "PlayerDestroyedEvent");
        assert_eq!(frame["guildId"], "1234");
    }
}
