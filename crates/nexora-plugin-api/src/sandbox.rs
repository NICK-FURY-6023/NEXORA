use std::net::IpAddr;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use nexora_error::{NexoraError, NexoraResult};

use crate::{PluginCapability, PluginManifest, SandboxPolicy};

/// A loaded plugin module and the policy it runs under.
#[derive(Debug, Clone)]
pub struct PluginModule {
    manifest: PluginManifest,
    policy: SandboxPolicy,
    entrypoint: PathBuf,
}

impl PluginModule {
    /// The manifest the module was loaded from.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// The sandbox policy derived from the manifest.
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    /// The resolved entrypoint path.
    pub fn entrypoint(&self) -> PathBuf {
        self.entrypoint.clone()
    }
}

/// Enforces a [`SandboxPolicy`] for one plugin.
///
/// Every external effect a plugin asks for goes through this type: the
/// sandbox decides whether the network host, the filesystem path, the
/// allocated memory and the elapsed time stay inside the policy.
#[derive(Debug, Clone)]
pub struct PluginSandbox {
    policy: SandboxPolicy,
    capabilities: Vec<PluginCapability>,
    started: Instant,
    allocated_bytes: u64,
}

impl PluginSandbox {
    /// Creates a sandbox for an explicit policy and capability set.
    pub fn new(policy: SandboxPolicy, capabilities: Vec<PluginCapability>) -> Self {
        Self {
            policy,
            capabilities,
            started: Instant::now(),
            allocated_bytes: 0,
        }
    }

    /// Creates a sandbox from a manifest's declared capabilities.
    pub fn from_manifest(manifest: &PluginManifest) -> NexoraResult<Self> {
        let policy = SandboxPolicy::from_manifest(manifest)?;
        Ok(Self::new(policy, manifest.capabilities.clone()))
    }

    /// The enforced policy.
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    /// The capability set exposed to the plugin.
    pub fn capabilities(&self) -> &[PluginCapability] {
        &self.capabilities
    }

    /// Bytes allocated by the plugin so far.
    pub fn allocated_bytes(&self) -> u64 {
        self.allocated_bytes
    }

    /// Shortens the time limit for the running plugin.
    pub fn set_time_limit_ms(&mut self, time_limit_ms: u64) {
        self.policy.time_limit_ms = time_limit_ms;
    }

    /// Shortens the memory limit for the running plugin.
    pub fn set_memory_limit_mb(&mut self, memory_limit_mb: u64) {
        self.policy.memory_limit_mb = memory_limit_mb;
    }

    /// Reports whether the plugin holds a capability.
    pub fn grants(&self, capability: &PluginCapability) -> bool {
        match capability {
            PluginCapability::Network(host) => {
                self.policy.network
                    && self
                        .policy
                        .allowed_hosts
                        .iter()
                        .any(|allowed| host_matches(allowed, host))
            }
            PluginCapability::FileSystem(path) => {
                self.policy.filesystem
                    && self
                        .policy
                        .allowed_paths
                        .iter()
                        .any(|allowed| path_matches(allowed, path))
            }
            _ => self.capabilities.contains(capability),
        }
    }

    /// Checks a network request against the policy.
    pub fn check_network(&self, url: &str) -> NexoraResult<()> {
        if !self.policy.network {
            return Err(NexoraError::service(
                "plugin sandbox denied the network access",
            ));
        }
        let host = host_of(url)
            .ok_or_else(|| NexoraError::invalid_input("plugin url must be an http(s) url"))?;
        if !self.grants(&PluginCapability::Network(host.clone())) {
            return Err(NexoraError::service(
                "plugin sandbox denied the network host",
            ));
        }
        if is_private_host(&host) {
            return Err(NexoraError::service(
                "plugin sandbox denied the network host",
            ));
        }
        Ok(())
    }

    /// Checks a filesystem path against the policy.
    pub fn check_path(&self, path: &str) -> NexoraResult<()> {
        if !self.policy.filesystem {
            return Err(NexoraError::service(
                "plugin sandbox denied the filesystem access",
            ));
        }
        let candidate = Path::new(path);
        if candidate.is_absolute() || candidate.has_root() {
            return Err(NexoraError::invalid_input(
                "plugin path must be relative to an allowed root",
            ));
        }
        if !candidate
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(NexoraError::service("plugin sandbox denied the path"));
        }
        if !self
            .policy
            .allowed_paths
            .iter()
            .any(|allowed| path_matches(allowed, path))
        {
            return Err(NexoraError::service("plugin sandbox denied the path"));
        }
        Ok(())
    }

    /// Accounts for an allocation against the memory limit.
    pub fn allocate(&mut self, bytes: u64) -> NexoraResult<()> {
        let limit = self.policy.memory_limit_mb.saturating_mul(1024 * 1024);
        let requested = self.allocated_bytes.saturating_add(bytes);
        if requested > limit {
            return Err(NexoraError::service("plugin sandbox denied the allocation"));
        }
        self.allocated_bytes = requested;
        Ok(())
    }

    /// Checks the elapsed time against the limit.
    pub fn check_time(&self) -> NexoraResult<()> {
        if self.started.elapsed().as_millis() > u128::from(self.policy.time_limit_ms) {
            return Err(NexoraError::service("plugin sandbox denied the operation"));
        }
        Ok(())
    }

    /// Loads a plugin module whose entrypoint lives inside `root`.
    pub fn load(&self, root: &Path, manifest: &PluginManifest) -> NexoraResult<PluginModule> {
        crate::validate_manifest(manifest)?;
        let entrypoint = manifest.entrypoint.replace('\\', "/");
        let relative = Path::new(&entrypoint);
        if relative.is_absolute()
            || !relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(NexoraError::invalid_input(
                "plugin entrypoint must stay inside the plugin root",
            ));
        }
        let path = root.join(relative);
        let metadata = std::fs::metadata(&path)
            .map_err(|_| NexoraError::invalid_input("plugin entrypoint is missing"))?;
        if !metadata.is_file() {
            return Err(NexoraError::invalid_input(
                "plugin entrypoint is not a file",
            ));
        }
        let limit = self.policy.memory_limit_mb.saturating_mul(1024 * 1024);
        if metadata.len() > limit {
            return Err(NexoraError::service("plugin sandbox denied the module"));
        }
        Ok(PluginModule {
            manifest: manifest.clone(),
            policy: self.policy.clone(),
            entrypoint: path,
        })
    }
}

fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or("");
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

fn host_matches(allowed: &str, host: &str) -> bool {
    let allowed = allowed.trim().to_ascii_lowercase();
    let host = host.trim().to_ascii_lowercase();
    if allowed.is_empty() || host.is_empty() {
        return false;
    }
    if let Some(suffix) = allowed.strip_prefix("*.") {
        host.len() > suffix.len()
            && host.ends_with(suffix)
            && host[..host.len() - suffix.len()].ends_with('.')
    } else {
        allowed == host
    }
}

fn path_matches(allowed: &str, path: &str) -> bool {
    let root = allowed.replace('\\', "/").trim_matches('/').to_owned();
    let normalized = path.replace('\\', "/");
    let candidate = normalized.trim_start_matches('/');
    if root.is_empty() {
        return false;
    }
    candidate == root || candidate.starts_with(&format!("{root}/"))
}

/// Reports whether a host points at the local network.
fn is_private_host(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    match host.parse::<IpAddr>() {
        Ok(address) => match address {
            IpAddr::V4(value) => {
                value.is_loopback()
                    || value.is_private()
                    || value.is_link_local()
                    || value.is_unspecified()
                    || value.is_multicast()
                    || value.is_broadcast()
            }
            IpAddr::V6(value) => {
                value.is_loopback()
                    || value.is_unspecified()
                    || value.is_multicast()
                    || (value.segments()[0] & 0xfe00) == 0xfc00
                    || (value.segments()[0] & 0xffc0) == 0xfe80
            }
        },
        Err(_) => false,
    }
}
