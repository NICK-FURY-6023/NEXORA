use serde::{Deserialize, Serialize};

use nexora_audio_types::AudioFormat;
use nexora_common::{ApiVersion, ApplicationId, ServiceName, SessionId, TimestampMs, TrackId};

pub mod v4;

pub use v4::{
    DEFAULT_RESUME_TIMEOUT_SECONDS, LAVALINK_V4_PROTOCOL_VERSION, V4ChannelMixFilter,
    V4CompressorFilter, V4DistortionFilter, V4EchoFilter, V4EqualizerBand, V4ErrorEnvelope,
    V4ErrorSeverity, V4EventMessage, V4Exception, V4Filters, V4Frame, V4Git, V4HighPassFilter,
    V4Info, V4KaraokeFilter, V4LoadResult, V4LoadType, V4LowPassFilter, V4Nullable, V4Op,
    V4PhaserFilter, V4Player, V4PlayerState, V4PlayerUpdate, V4PlayerUpdateFrame,
    V4PlayerUpdateTrack, V4Players, V4Playlist, V4PlaylistInfo, V4Plugin, V4Plugins, V4Ready,
    V4RotationFilter, V4SearchResult, V4Session, V4SessionUpdate, V4StatsFrame, V4TimescaleFilter,
    V4Track, V4TrackEndReason, V4TrackInfo, V4TremoloFilter, V4Version, V4VibratoFilter,
    V4VoiceState, check_protocol_version, decode_binary_track, decode_track, encode_binary_track,
    encode_track,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub sources: Option<Vec<String>>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub application: ApplicationId,
    pub version: String,
    pub output: AudioFormat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSessionResponse {
    pub session_id: SessionId,
    pub status: String,
    pub engine: String,
    pub api_version: ApiVersion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: SessionId,
    pub status: String,
    pub position_ms: u64,
    pub volume: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayRequest {
    pub track_id: TrackId,
    pub start_position_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthStatus {
    pub service: ServiceName,
    pub healthy: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventType {
    SessionCreated,
    SessionReady,
    TrackStarted,
    TrackEnded,
    TrackFailed,
    TrackPosition,
    PlaybackPaused,
    PlaybackResumed,
    PlaybackStopped,
    QueueUpdated,
    SourceUnavailable,
    EngineWarning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: String,
    pub version: ApiVersion,
    pub timestamp: TimestampMs,
    pub session_id: SessionId,
    pub event_type: SessionEventType,
    pub payload: serde_json::Value,
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    CreateSession(CreateSessionRequest),
    Play(PlayRequest),
    Pause {
        session_id: SessionId,
    },
    Resume {
        session_id: SessionId,
    },
    Stop {
        session_id: SessionId,
    },
    Seek {
        session_id: SessionId,
        position_ms: u64,
    },
    SetVolume {
        session_id: SessionId,
        volume: u8,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexora_audio_types::AudioCodec;

    fn sample_format() -> AudioFormat {
        AudioFormat::new(48_000, 2, AudioCodec::Opus)
    }

    fn roundtrip<T>(value: &T)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        let decoded: T = serde_json::from_str(&json).unwrap();
        assert_eq!(&decoded, value);
    }

    #[test]
    fn session_dtos_roundtrip() {
        let session_id = SessionId::new("nx_session_1").unwrap();
        let application = ApplicationId::new("my-bot").unwrap();
        let create = CreateSessionRequest {
            application,
            version: "1.0.0".into(),
            output: sample_format(),
        };
        roundtrip(&create);
        let response = CreateSessionResponse {
            session_id: session_id.clone(),
            status: "created".into(),
            engine: "nexora".into(),
            api_version: ApiVersion::V1,
        };
        roundtrip(&response);
        let state = SessionState {
            session_id,
            status: "ready".into(),
            position_ms: 0,
            volume: 100,
        };
        roundtrip(&state);
    }

    #[test]
    fn play_and_health_roundtrip() {
        let play = PlayRequest {
            track_id: TrackId::new("track_1").unwrap(),
            start_position_ms: 0,
        };
        roundtrip(&play);
        let health = HealthStatus {
            service: ServiceName::Gateway,
            healthy: true,
            message: "skeleton".into(),
        };
        roundtrip(&health);
    }

    #[test]
    fn event_envelope_roundtrip() {
        let event = EventEnvelope {
            event_id: "evt_1".into(),
            version: ApiVersion::V1,
            timestamp: TimestampMs(1_700_000_000_000),
            session_id: SessionId::new("nx_session_1").unwrap(),
            event_type: SessionEventType::TrackStarted,
            payload: serde_json::json!({ "track_id": "track_1" }),
            correlation_id: "corr_1".into(),
        };
        roundtrip(&event);
    }

    #[test]
    fn command_roundtrip() {
        let command = Command::Pause {
            session_id: SessionId::new("nx_session_1").unwrap(),
        };
        roundtrip(&command);
    }
}
