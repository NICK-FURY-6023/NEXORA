# NEXORA Development

## Toolchain

Rust 1.100.0 is pinned in `rust-toolchain.toml`. Components: `rustfmt`, `clippy`. Edition: 2024.

## Baseline commands

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Running the gateway skeleton

```bash
cargo run -p nexora-gateway -- --log-filter debug
```

Optional config file:

```bash
cargo run -p nexora-gateway -- --config ./config.toml
```

The Phase 1 gateway is a skeleton: it loads configuration, starts tracing, waits for Ctrl-C, and shuts down gracefully. It does not open HTTP/WebSocket/gRPC listeners yet.

## Configuration precedence

1. Built-in defaults in `nexora-config`.
2. Optional TOML file passed with `--config`.
3. Explicit environment variables such as `NEXORA_SERVER__PORT` and `NEXORA_AUDIO__SAMPLE_RATE`.

Invalid values are rejected during `NexoraConfig::validate()`.

## Dependency policy

Direct dependencies are pinned with exact versions in the root `Cargo.toml`; transitive dependencies are controlled by the committed `Cargo.lock`. Add a dependency only with a technical reason, license check, MSRV check, and phase justification.
