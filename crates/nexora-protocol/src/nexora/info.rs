use serde::{Deserialize, Serialize};

use nexora_error::NexoraError;

/// The Nexora protocol generation NEXORA implements.
pub const NEXORA_PROTOCOL_VERSION: &str = "4.0.0";

/// Version block of the Nexora nexora `/nexora/info` payload.
///
/// The reference carries a semver string and its three numeric parts plus an
/// optional pre-release tag (bytecode evidence on `Version` in
/// `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraVersion {
    pub semver: String,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    #[serde(rename = "preRelease")]
    pub pre_release: Option<String>,
}

impl NexoraVersion {
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
        }
    }
}

/// Git block of the Nexora nexora `/nexora/info` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraGit {
    pub branch: String,
    pub commit: String,
    #[serde(rename = "commitTime")]
    pub commit_time: i64,
}

/// One plugin entry of the Nexora nexora `/nexora/info` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraPlugin {
    pub name: String,
    pub path: String,
    pub version: String,
}

/// The plugin block of the Nexora nexora `/nexora/info` payload.
///
/// The reference nests the plugin list under a `plugins` object, not a bare
/// array (bytecode evidence on `Plugins` in `protocol-jvm-4.2.2.jar`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraPlugins {
    pub plugins: Vec<NexoraPlugin>,
}

/// The Nexora nexora `/nexora/info` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraInfo {
    pub name: String,
    pub version: NexoraVersion,
    #[serde(rename = "buildTime")]
    pub build_time: i64,
    pub git: NexoraGit,
    pub jvm: Option<String>,
    pub lavaplayer: Option<String>,
    #[serde(rename = "sourceManagers")]
    pub source_managers: Vec<String>,
    pub filters: Vec<String>,
    pub plugins: NexoraPlugins,
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
            NexoraError::invalid_input("unsupported Nexora protocol version".to_owned())
        })?;
    if major == 4 {
        Ok(())
    } else {
        Err(NexoraError::invalid_input(
            "unsupported Nexora protocol version".to_owned(),
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
    fn info_serialises_the_nexora_shape() {
        let info = NexoraInfo {
            name: "NEXORA".into(),
            version: NexoraVersion::nexora(),
            build_time: 1,
            git: NexoraGit {
                branch: "main".into(),
                commit: "abc".into(),
                commit_time: 1,
            },
            jvm: None,
            lavaplayer: Some("nexora-audio-engine".into()),
            source_managers: vec!["local".into()],
            filters: Vec::new(),
            plugins: NexoraPlugins::default(),
        };
        let value = serde_json::to_value(&info).unwrap();
        assert_eq!(value["name"], "NEXORA");
        assert!(value["version"]["semver"].is_string());
        assert!(value["version"]["preRelease"].is_null());
        assert_eq!(value["sourceManagers"][0], "local");
        assert_eq!(value["filters"].as_array().unwrap().len(), 0);
        assert!(value["plugins"]["plugins"].is_array());
    }

    #[test]
    fn version_negotiation_accepts_nexora_and_rejects_others() {
        assert!(check_protocol_version("4").is_ok());
        assert!(check_protocol_version("4.0.0").is_ok());
        assert!(check_protocol_version("4.9.9").is_ok());
        assert!(check_protocol_version("3.0.0").is_err());
        assert!(check_protocol_version("5.0.0").is_err());
        assert!(check_protocol_version("").is_err());
    }
}
