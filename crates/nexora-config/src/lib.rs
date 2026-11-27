use std::env;
use std::path::Path;
use std::str::FromStr;

use figment::Figment;
use figment::providers::{Format, Serialized, Toml};
use nexora_error::{NexoraError, NexoraResult};
use serde::{Deserialize, Serialize};

/// Complete NEXORA service configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NexoraConfig {
    /// Network server settings.
    pub server: ServerConfig,
    /// Processing engine settings.
    pub engine: EngineConfig,
    /// Audio defaults.
    pub audio: AudioConfig,
    /// Cache settings.
    pub cache: CacheConfig,
    /// Cluster settings.
    pub cluster: ClusterConfig,
    /// Plugin settings.
    pub plugins: PluginsConfig,
}

/// Network server configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Address on which the server listens.
    pub host: String,
    /// TCP port on which the server listens.
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_owned(),
            port: 8080,
        }
    }
}

/// Processing engine configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineConfig {
    /// Number of worker threads used by the engine.
    pub worker_threads: usize,
    /// Maximum number of concurrent sessions.
    pub max_sessions: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            worker_threads: 4,
            max_sessions: 100,
        }
    }
}

/// Audio configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Codec selected when a stream does not specify one.
    pub default_codec: String,
    /// Sample rate in hertz.
    pub sample_rate: u32,
    /// Number of audio channels.
    pub channels: u16,
    /// Whether the raw PCM streaming endpoint is served.
    ///
    /// The endpoint hands the decoded audio to the client, which is a lot of
    /// bandwidth, so it stays off unless it is asked for.
    pub raw_streaming: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            default_codec: "opus".to_owned(),
            sample_rate: 48_000,
            channels: 2,
            raw_streaming: false,
        }
    }
}

/// Cache configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Whether caching is enabled.
    pub enabled: bool,
    /// Maximum cache memory in mebibytes.
    pub max_memory_mb: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_memory_mb: 128,
        }
    }
}

/// Cluster configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClusterConfig {
    /// Whether cluster mode is enabled.
    pub enabled: bool,
    /// Role assigned to this instance.
    pub role: String,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            role: "standalone".to_owned(),
        }
    }
}

/// Plugin configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginsConfig {
    /// Whether plugins are enabled.
    pub enabled: bool,
    /// Whether plugins run in a sandbox.
    pub sandbox: bool,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sandbox: true,
        }
    }
}

impl NexoraConfig {
    /// Loads defaults, an optional TOML file, and explicit environment variable
    /// overrides.
    ///
    /// Environment variables use the `NEXORA_` prefix and `__` separator, for
    /// example `NEXORA_SERVER__PORT`.
    pub fn load(path: Option<&Path>) -> NexoraResult<Self> {
        Self::load_with_env_getter(path, &|name| env::var(name).ok())
    }

    fn load_with_env_getter<F>(path: Option<&Path>, getter: &F) -> NexoraResult<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut figment = Figment::from(Serialized::defaults(Self::default()));

        if let Some(path) = path {
            figment = figment.merge(Toml::file(path));
        }

        let mut config = figment.extract::<Self>().map_err(|error| {
            NexoraError::config_with_source("failed to load configuration", error)
        })?;
        apply_env_overrides(&mut config, getter)?;
        config.validate()?;

        Ok(config)
    }

    /// Validates all configuration values.
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

fn apply_env_string<F>(getter: &F, name: &str, target: &mut String)
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(value) = getter(name) {
        *target = value;
    }
}

fn apply_env_value<T, F>(
    getter: &F,
    name: &str,
    target: &mut T,
    invalid_context: &'static str,
) -> NexoraResult<()>
where
    T: FromStr,
    F: Fn(&str) -> Option<String>,
{
    if let Some(value) = getter(name) {
        *target = value
            .parse::<T>()
            .map_err(|_| NexoraError::config(invalid_context))?;
    }

    Ok(())
}

fn apply_env_overrides<F>(config: &mut NexoraConfig, getter: &F) -> NexoraResult<()>
where
    F: Fn(&str) -> Option<String>,
{
    apply_env_string(getter, "NEXORA_SERVER__HOST", &mut config.server.host);
    apply_env_value(
        getter,
        "NEXORA_SERVER__PORT",
        &mut config.server.port,
        "NEXORA_SERVER__PORT must be a valid port number",
    )?;
    apply_env_value(
        getter,
        "NEXORA_ENGINE__WORKER_THREADS",
        &mut config.engine.worker_threads,
        "NEXORA_ENGINE__WORKER_THREADS must be a valid integer",
    )?;
    apply_env_value(
        getter,
        "NEXORA_ENGINE__MAX_SESSIONS",
        &mut config.engine.max_sessions,
        "NEXORA_ENGINE__MAX_SESSIONS must be a valid integer",
    )?;
    apply_env_string(
        getter,
        "NEXORA_AUDIO__DEFAULT_CODEC",
        &mut config.audio.default_codec,
    );
    apply_env_value(
        getter,
        "NEXORA_AUDIO__SAMPLE_RATE",
        &mut config.audio.sample_rate,
        "NEXORA_AUDIO__SAMPLE_RATE must be a valid integer",
    )?;
    apply_env_value(
        getter,
        "NEXORA_AUDIO__CHANNELS",
        &mut config.audio.channels,
        "NEXORA_AUDIO__CHANNELS must be a valid integer",
    )?;
    apply_env_value(
        getter,
        "NEXORA_CACHE__ENABLED",
        &mut config.cache.enabled,
        "NEXORA_CACHE__ENABLED must be true or false",
    )?;
    apply_env_value(
        getter,
        "NEXORA_CACHE__MAX_MEMORY_MB",
        &mut config.cache.max_memory_mb,
        "NEXORA_CACHE__MAX_MEMORY_MB must be a valid integer",
    )?;
    apply_env_value(
        getter,
        "NEXORA_CLUSTER__ENABLED",
        &mut config.cluster.enabled,
        "NEXORA_CLUSTER__ENABLED must be true or false",
    )?;
    apply_env_string(getter, "NEXORA_CLUSTER__ROLE", &mut config.cluster.role);
    apply_env_value(
        getter,
        "NEXORA_PLUGINS__ENABLED",
        &mut config.plugins.enabled,
        "NEXORA_PLUGINS__ENABLED must be true or false",
    )?;
    apply_env_value(
        getter,
        "NEXORA_PLUGINS__SANDBOX",
        &mut config.plugins.sandbox,
        "NEXORA_PLUGINS__SANDBOX must be true or false",
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use nexora_error::ErrorCode;

    use super::NexoraConfig;

    static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(0);

    fn unique_toml_path() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let file_id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);

        std::env::temp_dir().join(format!(
            "nexora-config-test-{}-{timestamp}-{file_id}.toml",
            std::process::id()
        ))
    }

    #[test]
    fn default_configuration_is_valid() {
        let config = NexoraConfig::default();

        assert!(config.validate().is_ok());
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.audio.default_codec, "opus");
    }

    #[test]
    fn toml_file_overrides_defaults() {
        let path = unique_toml_path();
        fs::write(
            &path,
            r#"
                [server]
                host = "127.0.0.1"
                port = 9090

                [engine]
                worker_threads = 6
                max_sessions = 200

                [audio]
                default_codec = "pcm_s16le"
                sample_rate = 44100
                channels = 1
            "#,
        )
        .expect("write temporary TOML file");

        let result = NexoraConfig::load_with_env_getter(Some(path.as_path()), &|_| None);
        let _ = fs::remove_file(&path);
        let config = result.expect("load TOML configuration");

        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.engine.worker_threads, 6);
        assert_eq!(config.engine.max_sessions, 200);
        assert_eq!(config.audio.default_codec, "pcm_s16le");
        assert_eq!(config.audio.sample_rate, 44_100);
        assert_eq!(config.audio.channels, 1);
        assert_eq!(config.cache.max_memory_mb, 128);
    }

    #[test]
    fn environment_variables_override_toml_values() {
        let path = unique_toml_path();
        fs::write(
            &path,
            r#"
                [server]
                host = "10.0.0.1"
                port = 9000
            "#,
        )
        .expect("write temporary TOML file");

        let values = HashMap::from([
            ("NEXORA_SERVER__HOST", "192.0.2.2"),
            ("NEXORA_SERVER__PORT", "9200"),
        ]);
        let result = NexoraConfig::load_with_env_getter(Some(path.as_path()), &|name| {
            values.get(name).map(|value| (*value).to_string())
        });
        let _ = fs::remove_file(&path);
        let config = result.expect("load configuration with environment overrides");

        assert_eq!(config.server.host, "192.0.2.2");
        assert_eq!(config.server.port, 9200);
    }

    #[test]
    fn environment_variables_override_defaults() {
        let values = HashMap::from([
            ("NEXORA_SERVER__HOST", "192.0.2.1"),
            ("NEXORA_SERVER__PORT", "9100"),
            ("NEXORA_ENGINE__WORKER_THREADS", "3"),
            ("NEXORA_ENGINE__MAX_SESSIONS", "77"),
            ("NEXORA_AUDIO__DEFAULT_CODEC", "pcm_s16le"),
            ("NEXORA_AUDIO__SAMPLE_RATE", "44100"),
            ("NEXORA_AUDIO__CHANNELS", "1"),
            ("NEXORA_CACHE__ENABLED", "false"),
            ("NEXORA_CACHE__MAX_MEMORY_MB", "32"),
            ("NEXORA_CLUSTER__ENABLED", "true"),
            ("NEXORA_CLUSTER__ROLE", "worker"),
            ("NEXORA_PLUGINS__ENABLED", "false"),
            ("NEXORA_PLUGINS__SANDBOX", "false"),
        ]);

        let config = NexoraConfig::load_with_env_getter(None, &|name| {
            values.get(name).map(|value| (*value).to_string())
        })
        .expect("load environment configuration");

        assert_eq!(config.server.host, "192.0.2.1");
        assert_eq!(config.server.port, 9100);
        assert_eq!(config.engine.worker_threads, 3);
        assert_eq!(config.engine.max_sessions, 77);
        assert_eq!(config.audio.default_codec, "pcm_s16le");
        assert_eq!(config.audio.sample_rate, 44_100);
        assert_eq!(config.audio.channels, 1);
        assert!(!config.cache.enabled);
        assert_eq!(config.cache.max_memory_mb, 32);
        assert!(config.cluster.enabled);
        assert_eq!(config.cluster.role, "worker");
        assert!(!config.plugins.enabled);
        assert!(!config.plugins.sandbox);
    }

    #[test]
    fn invalid_environment_values_return_an_error_without_panicking() {
        let result = std::panic::catch_unwind(|| {
            NexoraConfig::load_with_env_getter(None, &|name| {
                if name == "NEXORA_SERVER__PORT" {
                    Some("not-a-port".to_owned())
                } else {
                    None
                }
            })
        });

        assert!(result.is_ok(), "configuration loading must not panic");
        let error = result
            .expect("load result should be available")
            .expect_err("invalid environment value should be rejected");
        assert_eq!(error.code(), ErrorCode::Config);
    }

    #[test]
    fn invalid_values_are_rejected() {
        let mut invalid_port = NexoraConfig::default();
        invalid_port.server.port = 0;

        let mut invalid_worker_threads = NexoraConfig::default();
        invalid_worker_threads.engine.worker_threads = 0;

        let mut invalid_max_sessions = NexoraConfig::default();
        invalid_max_sessions.engine.max_sessions = 0;

        let mut invalid_sample_rate = NexoraConfig::default();
        invalid_sample_rate.audio.sample_rate = 7_999;

        let mut invalid_channel_count = NexoraConfig::default();
        invalid_channel_count.audio.channels = 9;

        let mut invalid_cache_memory = NexoraConfig::default();
        invalid_cache_memory.cache.max_memory_mb = 0;

        let invalid_configs = [
            invalid_port,
            invalid_worker_threads,
            invalid_max_sessions,
            invalid_sample_rate,
            invalid_channel_count,
            invalid_cache_memory,
        ];

        for config in invalid_configs {
            let error = config.validate().expect_err("invalid configuration");
            assert_eq!(error.code(), ErrorCode::Config);
        }
    }

    #[test]
    fn invalid_file_values_return_an_error_without_panicking() {
        let path = unique_toml_path();
        fs::write(
            &path,
            r#"
                [server]
                port = 0
            "#,
        )
        .expect("write temporary TOML file");

        let no_environment = |_: &str| -> Option<String> { None };
        let result = std::panic::catch_unwind(|| {
            NexoraConfig::load_with_env_getter(Some(path.as_path()), &no_environment)
        });
        let _ = fs::remove_file(&path);

        assert!(result.is_ok(), "configuration loading must not panic");
        assert!(result.expect("load result should be available").is_err());
    }
}
