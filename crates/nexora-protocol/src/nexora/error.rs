use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Severity of a Nexora nexora exception.
///
/// The shipped reference names the three levels `COMMON`, `SUSPICIOUS` and
/// `FAULT` as the enum constants, but serialises them in lower case:
/// `common`, `suspicious`, `fault`
/// (bytecode evidence on `Exception$Severity` in `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NexoraErrorSeverity {
    Common,
    Suspicious,
    Fault,
}

impl NexoraErrorSeverity {
    /// The value the reference puts on the wire.
    pub fn as_str(&self) -> &'static str {
        match self {
            NexoraErrorSeverity::Common => "common",
            NexoraErrorSeverity::Suspicious => "suspicious",
            NexoraErrorSeverity::Fault => "fault",
        }
    }
}

/// The exception payload of the Nexora nexora protocol.
///
/// The reference carries a message, a severity, the root cause's class name
/// and the root cause's stack trace (bytecode evidence on `Exception` in
/// `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraException {
    pub message: String,
    pub severity: NexoraErrorSeverity,
    pub cause: Option<String>,
    #[serde(rename = "causeStackTrace")]
    pub cause_stack_trace: Option<String>,
}

impl NexoraException {
    /// A `fault`-severity exception with no root cause.
    pub fn fault(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            severity: NexoraErrorSeverity::Fault,
            cause: None,
            cause_stack_trace: None,
        }
    }

    /// Attach the root cause's class name and stack trace.
    pub fn with_cause(mut self, cause: impl Into<String>, stack_trace: Option<String>) -> Self {
        self.cause = Some(cause.into());
        self.cause_stack_trace = stack_trace;
        self
    }
}

/// The whole body Nexora nexora returns for a failed request.
///
/// The reference uses the server's own error shape: a timestamp, the HTTP
/// status, the status text, a trace, a message and the request path
/// (bytecode evidence on `Error` in `protocol-jvm-4.2.2.jar`).
///
/// The timestamp is epoch milliseconds, not an ISO-8601 string: the reference
/// annotates the field with its own `TimestampSerializer`, which writes
/// `Instant.toEpochMilliseconds()` through `encodeLong` (bytecode evidence on
/// `serialization/TimestampSerializer` in `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraErrorEnvelope {
    pub timestamp: u64,
    pub status: u16,
    pub error: String,
    pub trace: Option<String>,
    pub message: String,
    pub path: String,
}

impl NexoraErrorEnvelope {
    /// Build an envelope for a failed request.
    pub fn new(
        status: u16,
        error: impl Into<String>,
        message: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0);
        Self {
            timestamp,
            status,
            error: error.into(),
            trace: None,
            message: message.into(),
            path: path.into(),
        }
    }

    /// Attach a stack trace to the envelope.
    pub fn with_trace(mut self, trace: impl Into<String>) -> Self {
        self.trace = Some(trace.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severities_serialise_in_lower_case() {
        assert_eq!(NexoraErrorSeverity::Common.as_str(), "common");
        assert_eq!(NexoraErrorSeverity::Suspicious.as_str(), "suspicious");
        assert_eq!(NexoraErrorSeverity::Fault.as_str(), "fault");

        let value = serde_json::to_value(NexoraErrorSeverity::Fault).unwrap();
        assert_eq!(value, "fault");
    }

    #[test]
    fn the_exception_payload_uses_the_reference_field_names() {
        let exception =
            NexoraException::fault("boom").with_cause("java.lang.Error", Some("at x".into()));
        let value = serde_json::to_value(&exception).unwrap();
        assert_eq!(value["message"], "boom");
        assert_eq!(value["severity"], "fault");
        assert_eq!(value["cause"], "java.lang.Error");
        assert_eq!(value["causeStackTrace"], "at x");
    }

    #[test]
    fn the_error_envelope_uses_the_reference_field_names() {
        let envelope =
            NexoraErrorEnvelope::new(400, "Bad Request", "invalid", "/nexora/loadtracks");
        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["status"], 400);
        assert_eq!(value["error"], "Bad Request");
        assert_eq!(value["message"], "invalid");
        assert_eq!(value["path"], "/nexora/loadtracks");
        // The reference serialises the timestamp as epoch milliseconds.
        assert!(value["timestamp"].as_u64().unwrap() > 0);
    }
}
