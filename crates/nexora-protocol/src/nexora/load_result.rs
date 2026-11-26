//! The Nexora nexora `/nexora/loadtracks` result.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::NexoraException;
use super::track::NexoraTrack;

/// The `loadType` discriminator of a load result.
///
/// The shipped reference names the constants `TRACK`, `PLAYLIST`, `SEARCH`,
/// `NONE` and `ERROR`, but serialises them in lower case: `track`, `playlist`,
/// `search`, `empty`, `error`
/// (bytecode evidence on `ResultStatus` in `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NexoraLoadType {
    Track,
    Playlist,
    Search,
    Empty,
    Error,
}

impl NexoraLoadType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NexoraLoadType::Track => "track",
            NexoraLoadType::Playlist => "playlist",
            NexoraLoadType::Search => "search",
            NexoraLoadType::Empty => "empty",
            NexoraLoadType::Error => "error",
        }
    }
}

/// The playlist header of a `playlist` load result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraPlaylistInfo {
    pub name: String,
    #[serde(rename = "selectedTrack")]
    pub selected_track: i64,
}

/// The `playlist` load result payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexoraPlaylist {
    pub info: NexoraPlaylistInfo,
    #[serde(rename = "pluginInfo")]
    pub plugin_info: Value,
    pub tracks: Vec<NexoraTrack>,
}

/// The `search` load result payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexoraSearchResult {
    pub tracks: Vec<NexoraTrack>,
}

/// The `/nexora/loadtracks` response body.
///
/// The reference always answers HTTP 200: a failure is reported in the body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexoraLoadResult {
    #[serde(rename = "loadType")]
    pub load_type: NexoraLoadType,
    pub data: Value,
}

impl NexoraLoadResult {
    /// A single track was loaded.
    pub fn track(track: NexoraTrack) -> Self {
        Self {
            load_type: NexoraLoadType::Track,
            data: serde_json::to_value(track).unwrap_or(Value::Null),
        }
    }

    /// A playlist was loaded.
    pub fn playlist(name: impl Into<String>, selected_track: i64, tracks: Vec<NexoraTrack>) -> Self {
        let playlist = NexoraPlaylist {
            info: NexoraPlaylistInfo {
                name: name.into(),
                selected_track,
            },
            plugin_info: Value::Object(serde_json::Map::new()),
            tracks,
        };
        Self {
            load_type: NexoraLoadType::Playlist,
            data: serde_json::to_value(playlist).unwrap_or(Value::Null),
        }
    }

    /// A search result was loaded.
    pub fn search(tracks: Vec<NexoraTrack>) -> Self {
        let result = NexoraSearchResult { tracks };
        Self {
            load_type: NexoraLoadType::Search,
            data: serde_json::to_value(result).unwrap_or(Value::Null),
        }
    }

    /// Nothing matched the identifier.
    pub fn empty() -> Self {
        Self {
            load_type: NexoraLoadType::Empty,
            data: Value::Null,
        }
    }

    /// The lookup failed.
    pub fn error(exception: NexoraException) -> Self {
        Self {
            load_type: NexoraLoadType::Error,
            data: serde_json::to_value(exception).unwrap_or(Value::Null),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexora::error::NexoraErrorSeverity;

    #[test]
    fn load_types_serialise_in_lower_case() {
        assert_eq!(NexoraLoadType::Track.as_str(), "track");
        assert_eq!(NexoraLoadType::Playlist.as_str(), "playlist");
        assert_eq!(NexoraLoadType::Search.as_str(), "search");
        assert_eq!(NexoraLoadType::Empty.as_str(), "empty");
        assert_eq!(NexoraLoadType::Error.as_str(), "error");

        let value = serde_json::to_value(NexoraLoadResult::empty()).unwrap();
        assert_eq!(value["loadType"], "empty");
        assert!(value["data"].is_null());
    }

    #[test]
    fn a_search_result_carries_its_tracks() {
        let result = NexoraLoadResult::search(Vec::new());
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["loadType"], "search");
        assert!(value["data"]["tracks"].as_array().unwrap().is_empty());
    }

    #[test]
    fn an_error_result_carries_the_exception() {
        let exception = NexoraException {
            message: "boom".into(),
            severity: NexoraErrorSeverity::Fault,
            cause: Some("java.lang.IllegalStateException".into()),
            cause_stack_trace: None,
        };
        let value = serde_json::to_value(NexoraLoadResult::error(exception)).unwrap();
        assert_eq!(value["loadType"], "error");
        assert_eq!(value["data"]["message"], "boom");
        assert_eq!(value["data"]["severity"], "fault");
    }
}
