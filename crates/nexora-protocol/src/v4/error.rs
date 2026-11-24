use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Severity of a Lavalink v4 error.
///
/// The shipped reference names the three levels `COMMON`, `SUSPICIOUS` and
/// `FAULT` (craft-anything evidence on `Exception$Severity` in
/// `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum V4ErrorSeverity {
    #[serde(rename = "COMMON")]
    Common,
    #[serde(rename = "SUSPICIOUS")]
    Suspicious,
    #[serde(rename = "FAULT")]
    Fault,
}

impl V4ErrorSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            V4ErrorSeverity::Common => "COMMON",
            V4ErrorSeverity::Suspicious => "SUSPICIOUS",
            V4ErrorSeverity::Fault => "FAULT",
        }
    }
}

/// The error payload of the Lavalink v4 error envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4Error {
    pub code: String,
    pub message: String,
    pub severity: V4ErrorSeverity,
    pub cause: Option<String>,
}

/// The whole body Lavalink v4 returns for a failed request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4ErrorEnvelope {
    pub timestamp: i64,
    pub error: V4Error,
}

impl V4ErrorEnvelope {
    /// Build an envelope stamped with the current unix time in milliseconds.
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        severity: V4ErrorSeverity,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as i64)
            .unwrap_or(0);
        Self {
            timestamp,
            error: V4Error {
                code: code.into(),
                message: message.into(),
                severity,
                cause: None,
            },
        }
    }

    /// Attach the underlying cause to the envelope.
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.error.cause = Some(cause.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_envelope_shape_is_code_message_severity_cause() {
        let envelope =
            V4ErrorEnvelope::new("TRACK_NOT_FOUND", "no such track", V4ErrorSeverity::Common);
        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["error"]["code"], "TRACK_NOT_FOUND");
        assert_eq!(value["error"]["message"], "no such track");
        assert_eq!(value["error"]["severity"], "COMMON");
        assert!(value["error"]["cause"].is_null());
        assert!(value["timestamp"].as_i64().unwrap() > 0);
    }

    #[test]
    fn severity_names_match_the_shipped_reference() {
        assert_eq!(V4ErrorSeverity::Common.as_str(), "COMMON");
        assert_eq!(V4ErrorSeverity::Suspicious.as_str(), "SUSPICIOUS");
        assert_eq!(V4ErrorSeverity::Fault.as_str(), "FAULT");
    }
}
