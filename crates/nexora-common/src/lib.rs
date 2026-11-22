use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use nexora_error::{NexoraError, NexoraResult};
use serde::{Deserialize, Serialize};

/// Maximum number of characters accepted in an identifier.
pub const MAX_IDENTIFIER_LENGTH: usize = 128;

fn validate_identifier(value: &str) -> NexoraResult<()> {
    if value.is_empty() {
        return Err(NexoraError::invalid_input("identifier must not be empty"));
    }

    if value.chars().count() > MAX_IDENTIFIER_LENGTH {
        return Err(NexoraError::invalid_input(
            "identifier must not exceed 128 characters",
        ));
    }

    if value
        .chars()
        .any(|character| !character.is_ascii() || character.is_ascii_control())
    {
        return Err(NexoraError::invalid_input(
            "identifier must contain only printable ASCII characters",
        ));
    }

    Ok(())
}

macro_rules! define_identifier {
    ($name:ident) => {
        /// A validated identifier used by NEXORA domain objects.
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(String);

        impl $name {
            /// Creates an identifier after validating its contents.
            pub fn new(value: impl Into<String>) -> NexoraResult<Self> {
                let value = value.into();
                validate_identifier(&value)?;
                Ok(Self(value))
            }

            /// Returns the identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consumes the identifier and returns its owned value.
            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = NexoraError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl FromStr for $name {
            type Err = NexoraError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

define_identifier!(SessionId);
define_identifier!(TrackId);
define_identifier!(ApplicationId);

/// Names of services that can participate in the NEXORA system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceName {
    /// Public API gateway.
    Gateway,
    /// Authentication service.
    Auth,
    /// Service registry.
    Registry,
    /// Service resolver.
    Resolver,
    /// Media source worker.
    SourceWorker,
    /// Media decoder.
    Decoder,
    /// Audio processing engine.
    AudioEngine,
    /// Session manager.
    Session,
    /// Delivery service.
    Delivery,
    /// Cache service.
    Cache,
    /// Cluster coordinator.
    Cluster,
    /// Plugin host.
    PluginHost,
    /// Observability service.
    Observability,
    /// Administrative service.
    Admin,
}

impl fmt::Display for ServiceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Gateway => "gateway",
            Self::Auth => "auth",
            Self::Registry => "registry",
            Self::Resolver => "resolver",
            Self::SourceWorker => "source_worker",
            Self::Decoder => "decoder",
            Self::AudioEngine => "audio_engine",
            Self::Session => "session",
            Self::Delivery => "delivery",
            Self::Cache => "cache",
            Self::Cluster => "cluster",
            Self::PluginHost => "plugin_host",
            Self::Observability => "observability",
            Self::Admin => "admin",
        };

        formatter.write_str(name)
    }
}

/// Version of the NEXORA public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApiVersion {
    /// Major version component.
    pub major: u32,
    /// Minor version component.
    pub minor: u32,
}

impl ApiVersion {
    /// Version 1 of the NEXORA public API.
    pub const V1: Self = Self { major: 1, minor: 0 };
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.minor == 0 {
            write!(formatter, "v{}", self.major)
        } else {
            write!(formatter, "v{}.{}", self.major, self.minor)
        }
    }
}

/// Unix timestamp expressed in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimestampMs(pub u64);

impl TimestampMs {
    /// Returns the current Unix timestamp in milliseconds.
    ///
    /// Clocks set before the Unix epoch are represented as zero.
    pub fn now() -> Self {
        let milliseconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        Self(u64::try_from(milliseconds).unwrap_or(u64::MAX))
    }

    /// Returns the timestamp as a Unix millisecond count.
    pub const fn as_millis(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TimestampMs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}ms", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApiVersion, ApplicationId, MAX_IDENTIFIER_LENGTH, ServiceName, SessionId, TimestampMs,
        TrackId,
    };

    #[test]
    fn identifiers_accept_valid_values() {
        let session_id = SessionId::new("session-123").expect("valid session identifier");
        let track_id = TrackId::new("track_456").expect("valid track identifier");
        let application_id =
            ApplicationId::new("application-789").expect("valid application identifier");

        assert_eq!(session_id.as_str(), "session-123");
        assert_eq!(track_id.as_ref(), "track_456");
        assert_eq!(application_id.to_string(), "application-789");
    }

    #[test]
    fn identifiers_reject_empty_values() {
        assert!(SessionId::new("").is_err());
        assert!(TrackId::new(String::new()).is_err());
        assert!(ApplicationId::new("").is_err());
    }

    #[test]
    fn identifiers_reject_values_over_the_length_limit() {
        let value = "a".repeat(MAX_IDENTIFIER_LENGTH + 1);
        assert!(SessionId::new(value).is_err());
    }

    #[test]
    fn identifiers_accept_values_at_the_length_limit() {
        let value = "a".repeat(MAX_IDENTIFIER_LENGTH);
        assert!(SessionId::new(value).is_ok());
    }

    #[test]
    fn identifiers_reject_non_printable_ascii() {
        assert!(SessionId::new("session\nid").is_err());
        assert!(TrackId::new("track\tid").is_err());
        assert!(ApplicationId::new("application\u{007f}id").is_err());
    }

    #[test]
    fn identifiers_reject_non_ascii_values() {
        assert!(SessionId::new("session-ü").is_err());
        assert!(TrackId::new("track-会话").is_err());
        assert!(ApplicationId::new("application-🙂").is_err());
    }

    #[test]
    fn identifiers_round_trip_through_serde() {
        let session_id = SessionId::new("session-123").expect("valid session identifier");
        let track_id = TrackId::new("track-456").expect("valid track identifier");
        let application_id =
            ApplicationId::new("application-789").expect("valid application identifier");

        let session_json =
            serde_json::to_string(&session_id).expect("serialize session identifier");
        let track_json = serde_json::to_string(&track_id).expect("serialize track identifier");
        let application_json =
            serde_json::to_string(&application_id).expect("serialize application identifier");

        assert_eq!(session_json, "\"session-123\"");
        assert_eq!(track_json, "\"track-456\"");
        assert_eq!(application_json, "\"application-789\"");
        assert_eq!(
            serde_json::from_str::<SessionId>(&session_json)
                .expect("deserialize session identifier"),
            session_id
        );
        assert_eq!(
            serde_json::from_str::<TrackId>(&track_json).expect("deserialize track identifier"),
            track_id
        );
        assert_eq!(
            serde_json::from_str::<ApplicationId>(&application_json)
                .expect("deserialize application identifier"),
            application_id
        );
    }

    #[test]
    fn serde_rejects_invalid_identifiers() {
        assert!(serde_json::from_str::<SessionId>("\"\"").is_err());
        assert!(serde_json::from_str::<TrackId>("\"track\\nid\"").is_err());
    }

    #[test]
    fn service_names_use_snake_case_for_serde_and_display() {
        let cases = [
            (ServiceName::Gateway, "gateway", "gateway"),
            (ServiceName::Auth, "auth", "auth"),
            (ServiceName::Registry, "registry", "registry"),
            (ServiceName::Resolver, "resolver", "resolver"),
            (ServiceName::SourceWorker, "source_worker", "source_worker"),
            (ServiceName::Decoder, "decoder", "decoder"),
            (ServiceName::AudioEngine, "audio_engine", "audio_engine"),
            (ServiceName::Session, "session", "session"),
            (ServiceName::Delivery, "delivery", "delivery"),
            (ServiceName::Cache, "cache", "cache"),
            (ServiceName::Cluster, "cluster", "cluster"),
            (ServiceName::PluginHost, "plugin_host", "plugin_host"),
            (ServiceName::Observability, "observability", "observability"),
            (ServiceName::Admin, "admin", "admin"),
        ];

        for (service, serde_name, display_name) in cases {
            let serialized = serde_json::to_string(&service).expect("serialize service name");
            assert_eq!(serialized, format!("\"{serde_name}\""));
            assert_eq!(service.to_string(), display_name);
            assert_eq!(
                serde_json::from_str::<ServiceName>(&serialized).expect("deserialize service name"),
                service
            );
        }
    }

    #[test]
    fn api_version_displays_v1() {
        assert_eq!(ApiVersion::V1.to_string(), "v1");
        assert_eq!(ApiVersion { major: 1, minor: 2 }.to_string(), "v1.2");
    }

    #[test]
    fn timestamp_round_trips_through_serde() {
        let timestamp = TimestampMs(1_725_000_000_000);
        let serialized = serde_json::to_string(&timestamp).expect("serialize timestamp");
        let deserialized =
            serde_json::from_str::<TimestampMs>(&serialized).expect("deserialize timestamp");

        assert_eq!(serialized, "1725000000000");
        assert_eq!(deserialized, timestamp);
        assert_eq!(deserialized.as_millis(), 1_725_000_000_000);
        assert!(TimestampMs::now().as_millis() > 0);
    }
}
