use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use nexora_error::{NexoraError, NexoraResult};
use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> NexoraResult<Self> {
                let value = value.into();
                if value.is_empty()
                    || value.len() > 128
                    || value.chars().any(|c| !c.is_ascii() || c.is_ascii_control())
                {
                    return Err(NexoraError::invalid_input(concat!(
                        stringify!($name),
                        " must be 1..=128 printable ASCII characters"
                    )));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.as_str())
            }
        }
    };
}

id_type!(SessionId);
id_type!(TrackId);
id_type!(ApplicationId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceName {
    Gateway,
    Auth,
    Registry,
    Resolver,
    SourceWorker,
    Decoder,
    AudioEngine,
    Session,
    Delivery,
    Cache,
    Cluster,
    PluginHost,
    Observability,
    Admin,
}

impl fmt::Display for ServiceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Gateway => "gateway",
            Self::Auth => "auth",
            Self::Registry => "registry",
            Self::Resolver => "resolver",
            Self::SourceWorker => "source-worker",
            Self::Decoder => "decoder",
            Self::AudioEngine => "audio-engine",
            Self::Session => "session",
            Self::Delivery => "delivery",
            Self::Cache => "cache",
            Self::Cluster => "cluster",
            Self::PluginHost => "plugin-host",
            Self::Observability => "observability",
            Self::Admin => "admin",
        };
        f.write_str(text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
}

impl ApiVersion {
    pub const V1: Self = Self { major: 1, minor: 0 };
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}", self.major, self.minor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimestampMs(pub u64);

impl TimestampMs {
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
        Self(millis)
    }
}

impl fmt::Display for TimestampMs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ms", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_validate_and_display() {
        let id = SessionId::new("nx_session_1").unwrap();
        assert_eq!(id.as_str(), "nx_session_1");
        assert_eq!(id.to_string(), "nx_session_1");
        assert!(SessionId::new("").is_err());
        assert!(TrackId::new("bad\nid").is_err());
        assert!(ApplicationId::new("a".repeat(129)).is_err());
    }

    #[test]
    fn service_names_and_versions_are_stable() {
        assert_eq!(ServiceName::AudioEngine.to_string(), "audio-engine");
        assert_eq!(ApiVersion::V1.to_string(), "v1.0");
        let json = serde_json::to_string(&ServiceName::SourceWorker).unwrap();
        assert_eq!(json, "\"source_worker\"");
    }

    #[test]
    fn timestamp_roundtrip() {
        let now = TimestampMs(1_700_000_000_000);
        let json = serde_json::to_string(&now).unwrap();
        let decoded: TimestampMs = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, now);
    }
}
