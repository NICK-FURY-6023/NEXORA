use serde::{Deserialize, Serialize};

/// Lavalink v4 WebSocket op codes.
///
/// The shipped reference encodes them as strings, not numbers:
/// `Message$Op` carries `Ready`, `Stats`, `PlayerUpdate` and `Event`
/// (bytecode evidence on `Message$Op` in `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum V4Op {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "stats")]
    Stats,
    #[serde(rename = "playerUpdate")]
    PlayerUpdate,
    #[serde(rename = "event")]
    Event,
}

impl V4Op {
    pub fn as_str(&self) -> &'static str {
        match self {
            V4Op::Ready => "ready",
            V4Op::Stats => "stats",
            V4Op::PlayerUpdate => "playerUpdate",
            V4Op::Event => "event",
        }
    }

    /// Exact-match lookup: an unknown op is `None`, never a guess.
    pub fn from_wire(value: &str) -> Option<V4Op> {
        match value {
            "ready" => Some(V4Op::Ready),
            "stats" => Some(V4Op::Stats),
            "playerUpdate" => Some(V4Op::PlayerUpdate),
            "event" => Some(V4Op::Event),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_wire_values_match_the_shipped_reference() {
        assert_eq!(V4Op::Ready.as_str(), "ready");
        assert_eq!(V4Op::Stats.as_str(), "stats");
        assert_eq!(V4Op::PlayerUpdate.as_str(), "playerUpdate");
        assert_eq!(V4Op::Event.as_str(), "event");
        assert_eq!(V4Op::from_wire("playerUpdate"), Some(V4Op::PlayerUpdate));
        assert_eq!(V4Op::from_wire("nope"), None);
    }

    #[test]
    fn op_serialises_to_its_wire_value() {
        assert_eq!(serde_json::to_value(V4Op::Ready).unwrap(), "ready");
        assert_eq!(
            serde_json::to_value(V4Op::PlayerUpdate).unwrap(),
            "playerUpdate"
        );
    }
}
