# NEXORA plugin API

`nexora-plugin-api` is the contract a plugin is compiled against and the
sandbox the host runs it in. It has no plugin engine dependency of its own:
the manifest, the capability set and the policy checks live here so the host
and the plugin agree on one contract.

## Manifest

```rust
use nexora_plugin_api::{PluginCapability, PluginManifest};

let manifest = PluginManifest::new("my-plugin", "0.1.0", "plugin.wasm")
    .with_capability(PluginCapability::AudioDecode)
    .with_capability(PluginCapability::Network("api.example.com".to_owned()))
    .with_capability(PluginCapability::FileSystem("data".to_owned()));
```

`validate_manifest` rejects an empty name, version or entrypoint, a name
longer than 128 characters, a version longer than 64 characters, a duplicated
capability and a capability with an empty host or path target.

The entrypoint is a path relative to the plugin root. `PluginSandbox::load`
resolves it, rejects an absolute or traversing entrypoint, and refuses a
module larger than the sandbox memory limit.

## Capabilities

| Capability | Meaning |
|---|---|
| `AudioDecode` | The plugin may decode audio. |
| `AudioEncode` | The plugin may encode audio. |
| `SourceAdapter` | The plugin may act as a source adapter. |
| `Network(host)` | The plugin may reach `host`. `*.example.com` matches a subdomain; a bare host matches exactly. |
| `FileSystem(path)` | The plugin may read below `path`, relative to the plugin root. |

## Sandbox policy

`SandboxPolicy::from_manifest` derives the policy from the manifest: no
network and no filesystem unless a capability asks for it, a five second time
limit and a 64 MiB memory limit by default, and the declared hosts and paths
as the allow lists. `SandboxPolicy::deny_all` grants nothing at all.

`PluginSandbox` enforces the policy:

```rust
use nexora_plugin_api::PluginSandbox;

let sandbox = PluginSandbox::from_manifest(&manifest)?;
sandbox.check_network("https://api.example.com/track")?;
sandbox.check_path("data/track.bin")?;
sandbox.allocate(64 * 1024)?;
sandbox.check_time()?;
```

What each check refuses:

- **Network** — no network capability, a host outside the allow list, a
  non-http(s) scheme, and any host that points at the local network
  (loopback, private, link-local, unspecified and multicast addresses, plus
  `localhost` and `.local` names). The allow list never overrides the local
  network rule.
- **Filesystem** — no filesystem capability, an absolute or rooted path, a
  path with a `..` component, and a path outside the declared roots.
- **Memory** — an allocation that pushes the plugin past its memory limit.
- **Time** — an operation attempted after the time limit is spent.

A denied operation returns a `NexoraError`: `ErrorCode::InvalidInput` for a
malformed argument, `ErrorCode::Service` for a policy denial.

## Not wired yet

The sandbox is the enforcement half of the runtime. The interpreter that
executes a WebAssembly module against it is not part of this crate yet: it
needs a pinned WebAssembly engine (wasmtime), which the shared tree does not
vendor. Until that lands, `PluginSandbox` guards the effects a plugin asks
for, and the host calls the plugin's entry point itself.
