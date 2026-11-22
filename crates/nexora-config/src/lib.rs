use std::env;
use std::path::Path;
use std::str::FromStr;

use figment::Figment;
use figment::providers::{Format, Serialized, Toml};
use nexora_error::{NexoraError, NexoraResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineConfig {
    pub worker_threads: usize,
    pub max_sessions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioConfig {
    pub default_codec: String,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_memory_mb: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub enabled: bool,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginsConfig {
    pub enabled: bool,
    pub sandbox: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraConfig {
    pub server: ServerConfig,
    pub engine: EngineConfig,
    pub audio: AudioConfig,
    pub cache: CacheConfig,
    pub cluster: ClusterConfig,
    pub plugins: PluginsConfig,
}

impl Default for NexoraConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".into(),
                port: 8080,
            },
            engine: EngineConfig {
                worker_threads: 4,
                max_sessions: 100,
            },
            audio: AudioConfig {
                default_codec: "opus".into(),
                sample_rate: 48_000,
                channels: 2,
            },
            cache: CacheConfig {
                enabled: true,
                max_memory_mb: 128,
            },
            cluster: ClusterConfig {
                enabled: false,
                role: "standalone".into(),
            },
            plugins: PluginsConfig {
                enabled: true,
                sandbox: true,
            },
        }
    }
}

impl NexoraConfig {
    /// Load defaults, then an optional TOML file, then explicit environment overrides.
    pub fn load(path: Option<&Path>) -> NexoraResult<Self> {
        let mut figment = Figment::from(Serialized::defaults(Self::default()));
        if let Some(path) = path {
            figment = figment.merge(Toml::file(path));
        }
        let mut config: Self = figment.extract().map_err(|error| {
            NexoraError::config(format!("configuration extract failed: {error}"))
        })?;
        apply_env_overrides(&mut config, &|name| env::var(name).ok())?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> NexoraResult<()> {
        if self.server.port == 0 {
            return Err(NexoraError::config("server.port must be greater than zero"));
        }
        if self.engine.worker_threads == 0 {
            return Err(NexoraError::config(
                "engine.worker_threads must be greater than zero",
            ));
        }
        if self.engine.max_sessions == 0 {
            return Err(NexoraError::config(
                "engine.max_sessions must be greater than zero",
            ));
        }
        if !(8_000..=192_000).contains(&self.audio.sample_rate) {
            return Err(NexoraError::config(
                "audio.sample_rate must be between 8000 and 192000",
            ));
        }
        if !(1..=8).contains(&self.audio.channels) {
            return Err(NexoraError::config(
                "audio.channels must be between 1 and 8",
            ));
        }
        if self.cache.max_memory_mb == 0 {
            return Err(NexoraError::config(
                "cache.max_memory_mb must be greater than zero",
            ));
        }
        Ok(())
    }
}

fn env_string<F: Fn(&str) -> Option<String>>(getter: &F, name: &str, target: &mut String) {
    if let Some(value) = getter(name) {
        *target = value;
    }
}

fn env_value<T: FromStr, F: Fn(&str) -> Option<String>>(
    getter: &F,
    name: &str,
    target: &mut T,
) -> NexoraResult<()> {
    if let Some(value) = getter(name) {
        *target = value
            .parse::<T>()
            .map_err(|_| NexoraError::config(format!("invalid value for {name}")))?;
    }
    Ok(())
}

fn apply_env_overrides<F: Fn(&str) -> Option<String>>(
    config: &mut NexoraConfig,
    getter: &F,
) -> NexoraResult<()> {
    env_string(getter, "NEXORA_SERVER__HOST", &mut config.server.host);
    env_value(getter, "NEXORA_SERVER__PORT", &mut config.server.port)?;
    env_value(
        getter,
        "NEXORA_ENGINE__WORKER_THREADS",
        &mut config.engine.worker_threads,
    )?;
    env_value(
        getter,
        "NEXORA_ENGINE__MAX_SESSIONS",
        &mut config.engine.max_sessions,
    )?;
    env_string(
        getter,
        "NEXORA_AUDIO__DEFAULT_CODEC",
        &mut config.audio.default_codec,
    );
    env_value(
        getter,
        "NEXORA_AUDIO__SAMPLE_RATE",
        &mut config.audio.sample_rate,
    )?;
    env_value(getter, "NEXORA_AUDIO__CHANNELS", &mut config.audio.channels)?;
    env_value(getter, "NEXORA_CACHE__ENABLED", &mut config.cache.enabled)?;
    env_value(
        getter,
        "NEXORA_CACHE__MAX_MEMORY_MB",
        &mut config.cache.max_memory_mb,
    )?;
    env_value(
        getter,
        "NEXORA_CLUSTER__ENABLED",
        &mut config.cluster.enabled,
    )?;
    env_string(getter, "NEXORA_CLUSTER__ROLE", &mut config.cluster.role);
    env_value(
        getter,
        "NEXORA_PLUGINS__ENABLED",
        &mut config.plugins.enabled,
    )?;
    env_value(
        getter,
        "NEXORA_PLUGINS__SANDBOX",
        &mut config.plugins.sandbox,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn defaults_are_valid() {
        let config = NexoraConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.audio.default_codec, "opus");
    }

    #[test]
    fn invalid_values_are_rejected() {
        let mut config = NexoraConfig::default();
        config.server.port = 0;
        assert!(config.validate().is_err());
        config.server.port = 8080;
        config.audio.channels = 9;
        assert!(config.validate().is_err());
    }

    #[test]
    fn toml_file_overrides_defaults() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("nexora-config-test-{unique}.toml"));
        fs::write(&path, "[server]\nport = 9090\nhost = \"127.0.0.1\"\n").unwrap();
        let config = NexoraConfig::load(Some(&path)).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.server.host, "127.0.0.1");
    }

    #[test]
    fn environment_overrides_are_applied() {
        let mut config = NexoraConfig::default();
        apply_env_overrides(&mut config, &|name| {
            (name == "NEXORA_SERVER__PORT").then(|| "7070".to_string())
        })
        .unwrap();
        assert_eq!(config.server.port, 7070);
    }
}
