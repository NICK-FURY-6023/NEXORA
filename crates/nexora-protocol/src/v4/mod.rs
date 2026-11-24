pub mod error;
pub mod info;
pub mod op;
pub mod track;

pub use error::{V4Error, V4ErrorEnvelope, V4ErrorSeverity};
pub use info::{
    LAVALINK_V4_PROTOCOL_VERSION, V4Git, V4Info, V4Plugin, V4Version, check_protocol_version,
};
pub use op::{V4EmittedEvent, V4EventType, V4Op};
pub use track::{V4Track, V4TrackInfo, decode_track, encode_track};
