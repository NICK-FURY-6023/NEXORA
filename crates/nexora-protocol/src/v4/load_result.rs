//! The Lavalink v4 `/v4/loadtracks` result.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::V4Exception;
use super::track::V4Track;

/// The `loadType` discriminator of a load result.
///
/// The shipped reference names the constants `TRACK`, `PLAYLIST`, `SEARCH`,
/// `NONE` and `ERROR`, but serialises them in lower case: `track`, `playlist`,
/// `search`, `empty`, `error`
/// (bytecode evidence on `ResultStatus` in `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum V4LoadType {
    Track,
    Playlist,
    Search,
    Empty,
    Error,
}

impl V4LoadType {
    pub fn as_str(&self) -> &'static str {
        match self {
            V4LoadType::Track => "track",
            V4LoadType::Playlist => "playlist",
            V4LoadType::Search => "search",
            V4LoadType::Empty => "empty",
            V4LoadType::Error => "error",
        }
    }
}

/// The playlist header of a `playlist` load result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4PlaylistInfo {
    pub name: String,
    #[serde(rename = "selectedTrack")]
    pub selected_track: i64,
}

/// The `playlist` load result payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct V4Playlist {
    pub info: V4PlaylistInfo,
    #[serde(rename = "pluginInfo")]
    pub plugin_info: Value,
    pub tracks: Vec<V4Track>,
}

/// The `search` load result payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct V4SearchResult {
    pub tracks: Vec<V4Track>,
}

/// The `/v4/loadtracks` response body.
///
/// The reference always answers HTTP 200: a failure is reported in the body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct V4LoadResult {
    #[serde(rename = "loadType")]
    pub load_type: V4LoadType,
    pub data: Value,
}

impl V4LoadResult {
    /// A single track was loaded.
    pub fn track(track: V4Track) -> Self {
        Self {
            load_type: V4LoadType::Track,
            data: serde_json::to_value(track).unwrap_or(Value::Null),
        }
    }

    /// A playlist was loaded.
    pub fn playlist(name: impl Into<String>, selected_track: i64, tracks: Vec<V4Track>) -> Self {
        let playlist = V4Playlist {
            info: V4PlaylistInfo {
                name: name.into(),
                selected_track,
            },
            plugin_info: Value::Object(serde_json::Map::new()),
            tracks,
        };
        Self {
            load_type: V4LoadType::Playlist,
            data: serde_json::to_value(playlist).unwrap_or(Value::Null),
        }
    }

    /// A search result was loaded.
    pub fn search(tracks: Vec<V4Track>) -> Self {
        let result = V4SearchResult { tracks };
        Self {
            load_type: V4LoadType::Search,
            data: serde_json::to_value(result).unwrap_or(Value::Null),
        }
    }

    /// Nothing matched the identifier.
    pub fn empty() -> Self {
        Self {
            load_type: V4LoadType::Empty,
            data: Value::Null,
        }
    }

    /// The lookup failed.
    pub fn error(exception: V4Exception) -> Self {
        Self {
            load_type: V4LoadType::Error,
            data: serde_json::to_value(exception).unwrap_or(Value::Null),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v4::error::V4ErrorSeverity;

    #[test]
    fn load_types_serialise_in_lower_case() {
        assert_eq!(V4LoadType::Track.as_str(), "track");
        assert_eq!(V4LoadType::Playlist.as_str(), "playlist");
        assert_eq!(V4LoadType::Search.as_str(), "search");
        assert_eq!(V4LoadType::Empty.as_str(), "empty");
        assert_eq!(V4LoadType::Error.as_str(), "error");

        let value = serde_json::to_value(V4LoadResult::empty()).unwrap();
        assert_eq!(value["loadType"], "empty");
        assert!(value["data"].is_null());
    }

    #[test]
    fn a_search_result_carries_its_tracks() {
        let result = V4LoadResult::search(Vec::new());
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["loadType"], "search");
        assert!(value["data"]["tracks"].as_array().unwrap().is_empty());
    }

    #[test]
    fn an_error_result_carries_the_exception() {
        let exception = V4Exception {
            message: "boom".into(),
            severity: V4ErrorSeverity::Fault,
            cause: Some("java.lang.IllegalStateException".into()),
            cause_stack_trace: None,
        };
        let value = serde_json::to_value(V4LoadResult::error(exception)).unwrap();
        assert_eq!(value["loadType"], "error");
        assert_eq!(value["data"]["message"], "boom");
        assert_eq!(value["data"]["severity"], "fault");
    }
}
