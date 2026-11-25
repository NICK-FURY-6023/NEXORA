//! The Lavalink v4 session object and its update body.

use serde::{Deserialize, Serialize};

/// The Lavalink v4 session object.
///
/// The reference's property is `timeoutSeconds` but it serialises under the
/// JSON name `timeout` (bytecode evidence on `Session` in
/// `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4Session {
    pub resuming: bool,
    pub timeout: u64,
}

impl Default for V4Session {
    fn default() -> Self {
        Self {
            resuming: false,
            timeout: DEFAULT_RESUME_TIMEOUT_SECONDS,
        }
    }
}

/// The reference's default resume timeout, in seconds.
pub const DEFAULT_RESUME_TIMEOUT_SECONDS: u64 = 60;

/// The request body of `PATCH /v4/sessions/{sessionId}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4SessionUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resuming: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn session_serialises_the_timeout_under_its_json_name() {
        let session = V4Session {
            resuming: true,
            timeout: 30,
        };
        let value = serde_json::to_value(session).unwrap();
        assert_eq!(value["resuming"], true);
        assert_eq!(value["timeout"], 30);
        assert!(value.get("timeoutSeconds").is_none());
    }

    #[test]
    fn session_update_accepts_the_reference_field_names() {
        let update: V4SessionUpdate =
            serde_json::from_value(json!({ "resuming": true, "timeout": 120 })).unwrap();
        assert_eq!(update.resuming, Some(true));
        assert_eq!(update.timeout, Some(120));
    }

    #[test]
    fn session_update_ignores_an_empty_body() {
        let update: V4SessionUpdate = serde_json::from_value(json!({})).unwrap();
        assert_eq!(update.resuming, None);
        assert_eq!(update.timeout, None);
    }
}
