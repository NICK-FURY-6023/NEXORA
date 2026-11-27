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
    bytes: Vec<u8>,
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

    /// The module bytes, already read and checked against the policy.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
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
    ///
    /// The exposed capability set is what the policy actually grants: a
    /// capability the policy refuses is not handed to the plugin.
    pub fn new(policy: SandboxPolicy, capabilities: Vec<PluginCapability>) -> Self {
        let capabilities = capabilities
            .into_iter()
            .filter(|capability| granted_by(&policy, capability))
            .collect();
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

    /// Creates a sandbox whose policy is the manifest's policy narrowed by a
    /// host-owned policy.
    ///
    /// A manifest states what a plugin asks for; the host decides what it
    /// gets. Hosts, paths and limits that the host does not approve are
    /// dropped.
    pub fn from_manifest_with(
        manifest: &PluginManifest,
        approved: &SandboxPolicy,
    ) -> NexoraResult<Self> {
        let mut policy = SandboxPolicy::from_manifest(manifest)?;
        policy.network &= approved.network;
        policy.filesystem &= approved.filesystem;
        policy.allowed_hosts.retain(|host| {
            approved
                .allowed_hosts
                .iter()
                .any(|allowed| host_matches(allowed, host))
        });
        policy.allowed_paths.retain(|path| {
            approved
                .allowed_paths
                .iter()
                .any(|allowed| path_matches(allowed, path))
        });
        policy.time_limit_ms = policy.time_limit_ms.min(approved.time_limit_ms);
        policy.memory_limit_mb = policy.memory_limit_mb.min(approved.memory_limit_mb);
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
        self.policy.time_limit_ms = self.policy.time_limit_ms.min(time_limit_ms);
    }

    /// Shortens the memory limit for the running plugin.
    pub fn set_memory_limit_mb(&mut self, memory_limit_mb: u64) {
        self.policy.memory_limit_mb = self.policy.memory_limit_mb.min(memory_limit_mb);
    }

    /// Reports whether the plugin holds a capability.
    pub fn grants(&self, capability: &PluginCapability) -> bool {
        match capability {
            PluginCapability::Network(_) | PluginCapability::FileSystem(_) => {
                granted_by(&self.policy, capability)
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
        if std::fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(NexoraError::invalid_input(
                "plugin entrypoint must stay inside the plugin root",
            ));
        }
        // Resolve symlinks first: a link inside the root may point outside it.
        let root = std::fs::canonicalize(root)
            .map_err(|_| NexoraError::invalid_input("plugin root is missing"))?;
        let resolved = std::fs::canonicalize(&path)
            .map_err(|_| NexoraError::invalid_input("plugin entrypoint is missing"))?;
        if !resolved.starts_with(&root) {
            return Err(NexoraError::invalid_input(
                "plugin entrypoint must stay inside the plugin root",
            ));
        }
        let metadata = std::fs::metadata(&resolved)
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
        let bytes = std::fs::read(&resolved)
            .map_err(|_| NexoraError::invalid_input("plugin entrypoint is missing"))?;
        if !is_webassembly(&bytes) {
            return Err(NexoraError::invalid_input(
                "plugin entrypoint is not a webassembly module",
            ));
        }
        Ok(PluginModule {
            manifest: manifest.clone(),
            policy: self.policy.clone(),
            entrypoint: resolved,
            bytes,
        })
    }
}

/// True when the bytes start with the WebAssembly header: the magic and
/// version 1. A file that is not a module never reaches an engine.
fn is_webassembly(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes.starts_with(b"\0asm\x01\x00\x00\x00")
}

/// Reports whether the effective policy grants a capability.
///
/// The network and filesystem rules are the ones the checks apply, so a
/// capability the sandbox would refuse is never granted or exposed.
fn granted_by(policy: &SandboxPolicy, capability: &PluginCapability) -> bool {
    match capability {
        PluginCapability::Network(host) => {
            policy.network
                && !is_private_host(host)
                && policy
                    .allowed_hosts
                    .iter()
                    .any(|allowed| host_matches(allowed, host))
        }
        PluginCapability::FileSystem(path) => {
            policy.filesystem
                && Path::new(path)
                    .components()
                    .all(|component| matches!(component, Component::Normal(_)))
                && policy
                    .allowed_paths
                    .iter()
                    .any(|allowed| path_matches(allowed, path))
        }
        _ => true,
    }
}

pub(crate) fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // A backslash ends the authority for an http(s) URL, so the host is what
    // precedes it. Reading the whole authority would see an allowed name
    // where the request would go somewhere else entirely.
    if authority.contains('\\') {
        return None;
    }
    let hostport = authority.rsplit('@').next().unwrap_or("");
    let host = match hostport.strip_prefix('[') {
        Some(bracketed) => bracketed.split(']').next().unwrap_or(""),
        None if hostport.matches(':').count() > 1 => hostport,
        None => hostport.split(':').next().unwrap_or(""),
    };
    if host.is_empty() {
        None
    } else {
        Some(host.trim_end_matches('.').to_ascii_lowercase())
    }
}

pub(crate) fn host_matches(allowed: &str, host: &str) -> bool {
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

/// True when the path sits below the allowed root.
///
/// The comparison is component-wise, so it agrees with the component check in
/// `check_path` on every platform.
fn path_matches(allowed: &str, path: &str) -> bool {
    let root: Vec<Component<'_>> = Path::new(allowed).components().collect();
    let candidate: Vec<Component<'_>> = Path::new(path).components().collect();
    if root.is_empty() || candidate.len() < root.len() {
        return false;
    }
    root.iter()
        .zip(candidate.iter())
        .all(|(left, right)| left == right)
}

fn numeric_v4(host: &str) -> Option<IpAddr> {
    let value = if let Some(hex) = host.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()?
    } else if host.bytes().all(|byte| byte.is_ascii_digit()) {
        host.parse::<u64>().ok()?
    } else {
        return None;
    };
    if value > u64::from(u32::MAX) {
        return None;
    }
    let value = value as u32;
    Some(IpAddr::V4(std::net::Ipv4Addr::new(
        (value >> 24) as u8,
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    )))
}

fn short_v4(host: &str) -> Option<IpAddr> {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.is_empty()
        || parts.len() > 3
        || !parts
            .iter()
            .all(|part| part.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let mut octets = [0u8; 4];
    for (index, part) in parts.iter().enumerate() {
        let value = part.parse::<u32>().ok()?;
        if value > 255 {
            return None;
        }
        octets[index] = value as u8;
    }
    if parts.len() < 3 {
        octets[3] = octets[2];
        octets[2] = octets[1];
        octets[1] = 0;
    }
    Some(IpAddr::V4(std::net::Ipv4Addr::from(octets)))
}

fn is_private_address(address: IpAddr) -> bool {
    match address {
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
                || value
                    .to_ipv4()
                    .is_some_and(|mapped| is_private_address(IpAddr::V4(mapped)))
        }
    }
}

/// Reports whether a host points at the local network.
pub(crate) fn is_private_host(host: &str) -> bool {
    let host = host.trim_end_matches('.');
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    if let Some(address) = numeric_v4(host).or_else(|| short_v4(host)) {
        return is_private_address(address);
    }
    match host.parse::<IpAddr>() {
        Ok(address) => is_private_address(address),
        Err(_) => false,
    }
}
