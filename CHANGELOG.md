# Changelog

All notable changes to NEXORA are documented here.

## [Unreleased]

### Added

- Phase 1 Rust foundation: workspace manifest, toolchain pin, CI baseline.
- `nexora-error`, `nexora-common`, `nexora-audio-types`, `nexora-protocol`, `nexora-config`, `nexora-telemetry`, and `nexora-service` crates with unit tests.
- `nexora-gateway` skeleton binary with config loading, structured logging, and graceful shutdown.
- Project documentation: architecture, development, API contract status, contribution guide, ADR index.

### Verified

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (22 unit tests passed)

### Not included

- Audio decoding, source adapters, REST/WebSocket/gRPC APIs, SDKs, cluster, plugins, benchmarks.
