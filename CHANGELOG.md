# Changelog

All notable changes to NEXORA are documented here.

## [Unreleased]

### Added - Phase 2 Audio Engine

- `nexora-audio-decode`, `nexora-audio-codec`, `nexora-audio-pipeline`, and `nexora-audio-engine`.
- Local decoding, WAV/Opus output, resampling, channel remapping, bounded queue, clipping detection.
- Criterion resampling benchmark and Opus partial-frame regression test.

### Added - Phase 1 Foundation

- Phase 1 Rust foundation: workspace manifest, toolchain pin, CI baseline.
- `nexora-error`, `nexora-common`, `nexora-audio-types`, `nexora-protocol`, `nexora-config`, `nexora-telemetry`, and `nexora-service` crates with unit tests.
- `nexora-gateway` skeleton binary with config loading, structured logging, and graceful shutdown.
- Project documentation: architecture, development, API contract status, contribution guide, ADR index.

### Verified

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (42 tests passed); end-to-end WAV to Ogg Opus conversion; short Criterion benchmark run

### Not included

- Source adapters, REST/WebSocket/gRPC APIs, SDKs, cluster, plugins, production hardening.
