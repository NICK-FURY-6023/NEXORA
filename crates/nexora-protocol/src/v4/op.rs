use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Lavalink v4 WebSocket op codes.
///
/// The shipped reference encodes them as strings, not numbers:
/// `Message$Op` carries `Ready`, `Stats`, `PlayerUpdate` and `Event`
/// (craft-anything evidence on `protocol-jvm-4.2.2.jar`).
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

/// Event names carried by a `V4Op::Event` frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum V4EventType {
    #[serde(rename = "TrackStartEvent")]
    TrackStart,
    #[serde(rename = "TrackEndEvent")]
    TrackEnd,
    #[serde(rename = "TrackExceptionEvent")]
    TrackException,
    #[serde(rename = "playerUpdate")]
    PlayerUpdate,
    #[serde(rename = "WebSocketClosedEvent")]
    WebSocketClosed,
}

impl V4EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            V4EventType::TrackStart => "TrackStartEvent",
            V4EventType::TrackEnd => "TrackEndEvent",
            V4EventType::TrackException => "TrackExceptionEvent",
            V4EventType::PlayerUpdate => "playerUpdate",
            V4EventType::WebSocketClosed => "WebSocketClosedEvent",
        }
    }

    /// Exact-match lookup: an unknown event name is `None`, never a guess.
    pub fn from_wire(value: &str) -> Option<V4EventType> {
        match value {
            "TrackStartEvent" => Some(V4EventType::TrackStart),
            "TrackEndEvent" => Some(V4EventType::TrackEnd),
            "TrackExceptionEvent" => Some(V4EventType::TrackException),
            "playerUpdate" => Some(V4EventType::PlayerUpdate),
            "WebSocketClosedEvent" => Some(V4EventType::WebSocketClosed),
            _ => None,
        }
    }
}

/// One frame of the Lavalink v4 WebSocket protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct V4EmittedEvent {
    pub op: V4Op,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<V4EventType>,
    pub data: Value,
}

/// Payload of the `ready` op frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4Ready {
    pub version: crate::v4::info::V4Version,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn emitted_event_serialises_op_and_omits_absent_event() {
        let event = V4EmittedEvent {
            op: V4Op::Event,
            event: Some(V4EventType::TrackStart),
            data: json!({"track": {}}),
        };
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["op"], "event");
        assert_eq!(value["event"], "TrackStartEvent");
        let no_event = V4EmittedEvent {
            op: V4Op::Ready,
            event: None,
            data: json!({}),
        };
        assert!(
            serde_json::to_value(&no_event)
                .unwrap()
                .get("event")
                .is_none()
        );
    }

    #[test]
    fn event_type_wire_values_match_the_reference() {
        assert_eq!(V4EventType::TrackStart.as_str(), "TrackStartEvent");
        assert_eq!(V4EventType::TrackEnd.as_str(), "TrackEndEvent");
        assert_eq!(V4EventType::TrackException.as_str(), "TrackExceptionEvent");
        assert_eq!(V4EventType::PlayerUpdate.as_str(), "playerUpdate");
        assert_eq!(
            V4EventType::WebSocketClosed.as_str(),
            "WebSocketClosedEvent"
        );
    }
}
