use serde::{Deserialize, Serialize};

/// Nexora nexora WebSocket op codes.
///
/// The shipped reference encodes them as strings, not numbers:
/// `Message$Op` carries `Ready`, `Stats`, `PlayerUpdate` and `Event`
/// (bytecode evidence on `Message$Op` in `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NexoraOp {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "stats")]
    Stats,
    #[serde(rename = "playerUpdate")]
    PlayerUpdate,
    #[serde(rename = "event")]
    Event,
}

impl NexoraOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            NexoraOp::Ready => "ready",
            NexoraOp::Stats => "stats",
            NexoraOp::PlayerUpdate => "playerUpdate",
            NexoraOp::Event => "event",
        }
    }

    /// Exact-match lookup: an unknown op is `None`, never a guess.
    pub fn from_wire(value: &str) -> Option<NexoraOp> {
        match value {
            "ready" => Some(NexoraOp::Ready),
            "stats" => Some(NexoraOp::Stats),
            "playerUpdate" => Some(NexoraOp::PlayerUpdate),
            "event" => Some(NexoraOp::Event),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_wire_values_match_the_shipped_reference() {
        assert_eq!(NexoraOp::Ready.as_str(), "ready");
        assert_eq!(NexoraOp::Stats.as_str(), "stats");
        assert_eq!(NexoraOp::PlayerUpdate.as_str(), "playerUpdate");
        assert_eq!(NexoraOp::Event.as_str(), "event");
        assert_eq!(NexoraOp::from_wire("playerUpdate"), Some(NexoraOp::PlayerUpdate));
        assert_eq!(NexoraOp::from_wire("nope"), None);
    }

    #[test]
    fn op_serialises_to_its_wire_value() {
        assert_eq!(serde_json::to_value(NexoraOp::Ready).unwrap(), "ready");
        assert_eq!(
            serde_json::to_value(NexoraOp::PlayerUpdate).unwrap(),
            "playerUpdate"
        );
    }
}
