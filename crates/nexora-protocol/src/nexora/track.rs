use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::nexora::filters::NexoraFilters;
use nexora_error::NexoraError;

/// Track metadata of the Nexora nexora track shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NexoraTrackInfo {
    pub identifier: String,
    #[serde(rename = "isSeekable")]
    pub is_seekable: bool,
    pub author: String,
    pub length: u64,
    #[serde(rename = "isStream")]
    pub is_stream: bool,
    pub position: u64,
    pub title: String,
    pub uri: Option<String>,
    #[serde(rename = "artworkUrl")]
    pub artwork_url: Option<String>,
    pub isrc: Option<String>,
    #[serde(rename = "sourceName")]
    pub source_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<NexoraFilters>,
}

/// A Nexora nexora track: the encoded handle plus its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexoraTrack {
    pub encoded: String,
    pub info: NexoraTrackInfo,
    #[serde(rename = "pluginInfo")]
    pub plugin_info: Value,
    #[serde(rename = "userData")]
    pub user_data: Value,
}

/// Encode a track into Nexora's binary encoded-track string.
///
/// The format is Lavaplayer's version-3 track-info frame, byte for byte; see
/// `track_codec` for the layout and `docs/research/nexora-source-architecture.md`
/// §6 for the ground truth.
pub fn encode_track(info: &NexoraTrackInfo) -> Result<String, NexoraError> {
    super::track_codec::encode_binary_track(info)
}

/// Decode an encoded-track string back into track metadata.
pub fn decode_track(encoded: &str) -> Result<NexoraTrackInfo, NexoraError> {
    super::track_codec::decode_binary_track(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    fn sample_info() -> NexoraTrackInfo {
        NexoraTrackInfo {
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
        }
    }

    #[test]
    fn encode_decode_round_trip_is_lossless() {
        let info = sample_info();
        let encoded = encode_track(&info).unwrap();
        let decoded = decode_track(&encoded).unwrap();
        assert_eq!(decoded.identifier, "tone.wav");
        assert_eq!(decoded.title, "Tone");
        assert_eq!(decoded.length, 100);
        assert_eq!(decoded.author, "artist");
        assert_eq!(decoded.source_name, "local");
        assert!(decoded.is_seekable);
    }

    #[test]
    fn the_encoded_handle_is_base64_of_a_binary_frame() {
        let encoded = encode_track(&sample_info()).unwrap();
        let bytes = STANDARD.decode(&encoded).unwrap();
        assert!(bytes.len() > 4);
        // The header's top two bits are the frame flags.
        assert_eq!(bytes[0] >> 6, 0b01);
    }

    #[test]
    fn decode_track_rejects_malformed_input() {
        assert!(decode_track("not base64!!").is_err());
        assert!(decode_track(&STANDARD.encode([0xff, 0xfe])).is_err());
    }
}
