use nexora_error::{NexoraError, NexoraResult};

mod runtime;
mod sandbox;

pub use runtime::PluginRuntime;
pub use sandbox::{PluginModule, PluginSandbox};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PluginCapability {
    AudioDecode,
    AudioEncode,
    SourceAdapter,
    Network(String),
    FileSystem(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub entrypoint: String,
    pub capabilities: Vec<PluginCapability>,
}

impl PluginManifest {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        entrypoint: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            entrypoint: entrypoint.into(),
            capabilities: Vec::new(),
        }
    }

    pub fn with_capability(mut self, capability: PluginCapability) -> Self {
        self.capabilities.push(capability);
        self
    }
}

pub fn validate_manifest(manifest: &PluginManifest) -> NexoraResult<()> {
    if manifest.name.is_empty()
        || manifest.version.is_empty()
        || manifest.entrypoint.is_empty()
        || manifest.name.len() > 128
        || manifest.version.len() > 64
    {
        return Err(NexoraError::invalid_input("invalid plugin manifest"));
    }
    for (index, capability) in manifest.capabilities.iter().enumerate() {
        if manifest.capabilities[index + 1..].contains(capability) {
            return Err(NexoraError::invalid_input("duplicate plugin capability"));
        }
        if matches!(capability, PluginCapability::Network(host) if host.is_empty())
            || matches!(capability, PluginCapability::FileSystem(path) if path.is_empty())
        {
            return Err(NexoraError::invalid_input(
                "capability target must not be empty",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub network: bool,
    pub filesystem: bool,
    pub time_limit_ms: u64,
    pub memory_limit_mb: u64,
    pub allowed_hosts: Vec<String>,
    pub allowed_paths: Vec<String>,
}

impl SandboxPolicy {
    /// A policy that grants nothing and allows no time or memory.
    pub fn deny_all() -> Self {
        Self {
            network: false,
            filesystem: false,
            time_limit_ms: 0,
            memory_limit_mb: 0,
            allowed_hosts: Vec::new(),
            allowed_paths: Vec::new(),
        }
    }

    pub fn from_manifest(manifest: &PluginManifest) -> NexoraResult<Self> {
        validate_manifest(manifest)?;
        let mut policy = Self {
            network: false,
            filesystem: false,
            time_limit_ms: 5_000,
            memory_limit_mb: 64,
            allowed_hosts: Vec::new(),
            allowed_paths: Vec::new(),
        };
        for capability in &manifest.capabilities {
            match capability {
                PluginCapability::Network(host) => {
                    policy.network = true;
                    policy.allowed_hosts.push(host.clone());
                }
                PluginCapability::FileSystem(path) => {
                    policy.filesystem = true;
                    policy.allowed_paths.push(path.clone());
                }
                _ => {}
            }
        }
        Ok(policy)
    }
}
