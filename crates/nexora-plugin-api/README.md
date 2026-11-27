# NEXORA plugin API

`nexora-plugin-api` is the contract a plugin is compiled against, the sandbox
the host runs it in, and the runtime that executes a module under that
sandbox. The manifest, the capability set and the policy checks live here so
the host and the plugin agree on one contract.

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
resolves it through symlinks, rejects an absolute, traversing or
out-of-root entrypoint, refuses a module larger than the sandbox memory
limit, and refuses a file that is not a WebAssembly module: the bytes must
start with the magic and version 1 (`\0asm\x01\x00\x00\x00`). The loaded
bytes are on the module, so the host hands the engine exactly what it
checked.

```rust
let module = sandbox.load(&root, &manifest)?;
assert_eq!(module.bytes(), b"\0asm\x01\x00\x00\x00");
```

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

## Host approval

A manifest states what a plugin asks for; the host decides what it gets.
`PluginSandbox::from_manifest_with` narrows the manifest's policy with a
host-owned one: hosts and paths the host did not approve are dropped, and the
time and memory limits can only go down.

```rust
let sandbox = PluginSandbox::from_manifest_with(&manifest, &host_policy)?;
```

The allow lists name hosts, not ports: approving `api.example.com` approves
that host on any port.

The capability set the sandbox exposes (`PluginSandbox::capabilities`) is the
set the effective policy actually grants. A host the host did not approve, a
host that points at the local network and a path outside the declared roots
are dropped from it, so a plugin is never shown a capability the sandbox
would refuse.

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

## Running a module

`PluginRuntime` executes a WebAssembly module under a policy. The module is
run by an interpreter that has no ambient authority: it reaches the network,
the filesystem and its own memory only through the host functions the runtime
installs, and every one of them is checked against the policy first.

```rust
use nexora_plugin_api::{PluginManifest, PluginRuntime, SandboxPolicy};

let mut policy = SandboxPolicy::deny_all();
policy.network = true;
policy.allowed_hosts = vec!["api.example.com".to_owned()];
policy.time_limit_ms = 1_000;

let runtime = PluginRuntime::with_policy(policy)?;
let manifest = PluginManifest::new("my-plugin", "0.1.0", "plugin.wasm");
let results = runtime.call(&manifest, &module_bytes, "answer", &[])?;
```

The arguments are the strings the host hands the module. A module receives
each one as the `i32` index of its place in the call, so it never has to own
the bytes: the host keeps them, and the host function the module calls reads
the one it was given.

A module hands strings back the same way. It calls `host.push(offset, len)`
with the range of its own memory that holds the text; the host reads the
bytes and keeps them, and the call returns the index the string was given.
The results are the module's own `i32` values plus the strings it pushed:

```rust
let call = runtime.call(&manifest, &module_bytes, "answer", &[])?;
call.results; // the module's i32 results
call.strings; // the strings it pushed, in the order it pushed them
```

The time limit is enforced as fuel: the interpreter charges for each unit of
work and the module stops when the fuel is spent, so a module that never
returns is stopped rather than hung. The memory limit is enforced by the
interpreter's own memory limiter. The strings a module pushes live in the
host, so they have a limit of their own: a call may push at most
`MAX_PUSHED_STRINGS` strings, and a module that pushes past that is refused.

A module that asks for an effect the policy refuses is told the effect did
not happen, and the host sees the refusal after the call.

## Not wired yet

`nexora-plugin-api` has no dependents: the server cannot load a plugin yet.
The runtime is ready for it to do so.

