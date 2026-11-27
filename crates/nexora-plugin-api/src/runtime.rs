use wasmi::{
    Config, Engine, FuncType, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Val, ValType,
};

use nexora_error::{NexoraError, NexoraResult};

use crate::{PluginManifest, SandboxPolicy};

/// The host module a plugin imports its effects from.
const HOST_MODULE: &str = "host";

/// Fuel charged for each unit of work, so a time limit maps onto fuel.
const FUEL_PER_MS: u64 = 2_000;

/// Runs a plugin module under a [`SandboxPolicy`].
///
/// The module is executed by an interpreter that has no ambient authority: it
/// reaches the network, the filesystem and its own memory only through the
/// host functions this type installs, and every one of them is checked
/// against the policy first.
pub struct PluginRuntime {
    policy: SandboxPolicy,
}

impl PluginRuntime {
    /// Creates a runtime with the policy a manifest asks for.
    pub fn new() -> NexoraResult<Self> {
        Ok(Self {
            policy: default_policy(),
        })
    }

    /// Creates a runtime with a host-owned policy.
    pub fn with_policy(policy: SandboxPolicy) -> NexoraResult<Self> {
        Ok(Self { policy })
    }

    /// Narrows the runtime's policy to what the host approves.
    pub fn with_policy_narrowed(mut self, approved: &SandboxPolicy) -> Self {
        self.policy.network &= approved.network;
        self.policy.filesystem &= approved.filesystem;
        self.policy
            .allowed_hosts
            .retain(|host| approved.allowed_hosts.iter().any(|allowed| allowed == host));
        self.policy
            .allowed_paths
            .retain(|path| approved.allowed_paths.iter().any(|allowed| allowed == path));
        self.policy.time_limit_ms = self.policy.time_limit_ms.min(approved.time_limit_ms);
        self.policy.memory_limit_mb = self.policy.memory_limit_mb.min(approved.memory_limit_mb);
        self
    }

    /// The policy the runtime enforces.
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    /// Runs an exported function of a module and returns its results.
    ///
    /// The arguments are the strings the host hands the module. A module
    /// receives each one as the `i32` index of its place in the call, so it
    /// never has to own the bytes: the host keeps them and the host function
    /// that the module calls reads the one it was given.
    ///
    /// The results are the module's own `i32` values, which is the only type a
    /// plugin may exchange with the host.
    pub fn call(
        &self,
        manifest: &PluginManifest,
        wasm: &[u8],
        export: &str,
        args: &[&str],
    ) -> NexoraResult<Vec<i32>> {
        crate::validate_manifest(manifest)?;
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, wasm)
            .map_err(|_| NexoraError::invalid_input("plugin module is not valid webassembly"))?;

        let limits = StoreLimitsBuilder::new()
            .memory_size(memory_limit_bytes(self.policy.memory_limit_mb))
            .build();
        let mut store = Store::new(
            &engine,
            HostState {
                policy: self.policy.clone(),
                limits: limits.clone(),
                arguments: args.iter().map(|arg| (*arg).to_owned()).collect(),
                denied: None,
            },
        );
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(self.policy.time_limit_ms.saturating_mul(FUEL_PER_MS))
            .map_err(|_| NexoraError::service("plugin sandbox denied the operation"))?;

        let mut linker = <Linker<HostState>>::new(&engine);
        install_host_functions(&mut linker)?;
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(map_error)?;

        let function = instance
            .get_func(&store, export)
            .ok_or_else(|| NexoraError::invalid_input("plugin module has no such export"))?;
        let ty = function.ty(&store);
        if ty.params().len() != args.len() {
            return Err(NexoraError::invalid_input(
                "plugin export takes a different number of arguments",
            ));
        }

        let params = (0..args.len())
            .map(|index| Val::I32(index as i32))
            .collect::<Vec<_>>();
        let mut results = vec![Val::I32(0); ty.results().len()];
        function
            .call(&mut store, &params, &mut results)
            .map_err(map_error)?;

        // A refused effect is reported after the call: the module is told the
        // effect did not happen, and the host still sees the refusal.
        if let Some(reason) = store.data().denied.clone() {
            return Err(NexoraError::service(reason));
        }
        Ok(results
            .into_iter()
            .map(|value| match value {
                Val::I32(number) => number,
                _ => 0,
            })
            .collect())
    }
}

impl Default for PluginRuntime {
    fn default() -> Self {
        Self {
            policy: default_policy(),
        }
    }
}

/// The policy a runtime starts with: no network, no filesystem, and the same
/// limits a manifest policy carries.
fn default_policy() -> SandboxPolicy {
    SandboxPolicy {
        network: false,
        filesystem: false,
        time_limit_ms: 5_000,
        memory_limit_mb: 64,
        allowed_hosts: Vec::new(),
        allowed_paths: Vec::new(),
    }
}

/// The state the host functions read while the module runs.
struct HostState {
    policy: SandboxPolicy,
    limits: StoreLimits,
    arguments: Vec<String>,
    denied: Option<String>,
}

/// Installs the host functions a plugin may import.
///
/// Each one checks the policy before the effect happens, so a module cannot
/// reach a host the sandbox refuses.
fn install_host_functions(linker: &mut Linker<HostState>) -> NexoraResult<()> {
    let network_type = FuncType::new([ValType::I32], [ValType::I32]);
    linker
        .func_new(
            HOST_MODULE,
            "fetch",
            network_type,
            |mut caller, args, results| {
                let index = match args.first() {
                    Some(Val::I32(index)) => *index,
                    _ => {
                        return Err(wasmi::Error::new("plugin host call needs an i32 argument"));
                    }
                };
                let state = caller.data_mut();
                let url = state
                    .arguments
                    .get(index.max(0) as usize)
                    .map(String::as_str)
                    .unwrap_or("");
                if let Err(error) = check_network(&state.policy, url) {
                    state.denied = Some(error.to_string());
                    results[0] = Val::I32(-1);
                    return Ok(());
                }
                results[0] = Val::I32(0);
                Ok(())
            },
        )
        .map_err(|_| NexoraError::service("plugin sandbox denied the operation"))?;
    Ok(())
}

/// Refuses a url the policy does not allow.
fn check_network(policy: &SandboxPolicy, url: &str) -> NexoraResult<()> {
    if !policy.network {
        return Err(NexoraError::service(
            "plugin sandbox denied the network access",
        ));
    }
    let host = crate::sandbox::host_of(url)
        .ok_or_else(|| NexoraError::invalid_input("plugin url must be an http(s) url"))?;
    if crate::sandbox::is_private_host(&host)
        || !policy
            .allowed_hosts
            .iter()
            .any(|allowed| crate::sandbox::host_matches(allowed, &host))
    {
        return Err(NexoraError::service(
            "plugin sandbox denied the network host",
        ));
    }
    Ok(())
}

/// The byte limit the memory limiter enforces.
fn memory_limit_bytes(limit_mb: u64) -> usize {
    usize::try_from(limit_mb.saturating_mul(1024 * 1024)).unwrap_or(usize::MAX)
}

/// Turns an interpreter error into the error the caller sees.
fn map_error(error: wasmi::Error) -> NexoraError {
    if error.as_trap_code().is_some() {
        return NexoraError::service("plugin sandbox denied the operation");
    }
    match error.kind() {
        wasmi::errors::ErrorKind::Fuel(_) => {
            NexoraError::service("plugin sandbox denied the operation")
        }
        _ => NexoraError::internal("plugin module failed to run"),
    }
}
