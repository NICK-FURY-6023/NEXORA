use serde::{Deserialize, Serialize};

use nexora_error::NexoraError;

/// The Lavalink protocol generation NEXORA implements.
pub const LAVALINK_V4_PROTOCOL_VERSION: &str = "4.0.0";

/// Version block of the Lavalink v4 `/v4/info` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4Version {
    pub semver: String,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    #[serde(rename = "preRelease")]
    pub pre_release: Option<String>,
    pub build: Option<String>,
}

impl V4Version {
    /// NEXORA's own version, shaped like the reference's version block.
    pub fn nexora() -> Self {
        let semver = env!("CARGO_PKG_VERSION").to_owned();
        let mut parts = semver.split('.');
        let major = parts.next().and_then(parse_number).unwrap_or(0);
        let minor = parts.next().and_then(parse_number).unwrap_or(0);
        let patch = parts.next().and_then(parse_number).unwrap_or(0);
        Self {
            semver,
            major,
            minor,
            patch,
            pre_release: None,
            build: None,
        }
    }
}

/// Git block of the Lavalink v4 `/v4/info` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4Git {
    pub branch: String,
    pub commit: String,
    #[serde(rename = "commitTime")]
    pub commit_time: i64,
}

/// One plugin entry of the Lavalink v4 `/v4/info` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4Plugin {
    pub name: String,
    pub path: String,
    pub version: String,
}

/// The Lavalink v4 `/v4/info` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4Info {
    pub name: String,
    pub version: V4Version,
    #[serde(rename = "buildTime")]
    pub build_time: i64,
    pub git: V4Git,
    pub jvm: Option<String>,
    pub lavaplayer: Option<String>,
    #[serde(rename = "sourceManagers")]
    pub source_managers: Vec<String>,
    pub filters: Vec<String>,
    pub plugins: Vec<V4Plugin>,
}

/// Accept only the protocol generation NEXORA implements.
///
/// A client asking for another generation gets a documented error instead of
/// a silently different payload shape.
pub fn check_protocol_version(requested: &str) -> Result<(), NexoraError> {
    let major = requested
        .split('.')
        .next()
        .and_then(parse_number)
        .ok_or_else(|| {
            NexoraError::invalid_input("unsupported Lavalink protocol version".to_owned())
        })?;
    if major == 4 {
        Ok(())
    } else {
        Err(NexoraError::invalid_input(
            "unsupported Lavalink protocol version".to_owned(),
        ))
    }
}

fn parse_number(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_serialises_the_v4_shape() {
        let info = V4Info {
            name: "NEXORA".into(),
            version: V4Version::nexora(),
            build_time: 1,
            git: V4Git {
                branch: "main".into(),
                commit: "abc".into(),
                commit_time: 1,
            },
            jvm: None,
            lavaplayer: Some("nexora-audio-engine".into()),
            source_managers: vec!["local".into()],
            filters: Vec::new(),
            plugins: Vec::new(),
        };
        let value = serde_json::to_value(&info).unwrap();
        assert_eq!(value["name"], "NEXORA");
        assert!(value["version"]["semver"].is_string());
        assert!(value["version"]["preRelease"].is_null());
        assert_eq!(value["sourceManagers"][0], "local");
        assert_eq!(value["filters"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn version_negotiation_accepts_v4_and_rejects_others() {
        assert!(check_protocol_version("4").is_ok());
        assert!(check_protocol_version("4.0.0").is_ok());
        assert!(check_protocol_version("4.9.9").is_ok());
        assert!(check_protocol_version("3.0.0").is_err());
        assert!(check_protocol_version("5.0.0").is_err());
        assert!(check_protocol_version("").is_err());
    }
}
