# Changelog

All notable changes to NEXORA are documented here.

## [Unreleased]

### Added - Phase 4b Internal gRPC

- `nexora-grpc` with `nexora.v1.SessionService` protobuf contract and tonic server.
- gRPC session lifecycle RPCs sharing the HTTP session manager.
- Live client example and integration test.

### Added - Phase 4a Hosted Server

- `nexora-session` and `nexora-server` with NEXORA REST/WebSocket API and Lavalink-compatible `/v4` surface.
- Bearer authorization, consistent API errors, session lifecycle events, and local-file Ogg Opus streaming.

### Added - Phase 3 Source Framework

- `nexora-source-api` and `nexora-source-framework`.
- Registry, ranking, timeouts, cancellation, health aggregation, and SSRF-safe URL validation.
- Local media, public radio, and authorized HTTP stream adapters.

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
- `cargo test --workspace` (63 tests passed); live REST, WebSocket, and gRPC smoke tests; end-to-end WAV to Ogg Opus conversion; short Criterion benchmark run

### Not included

- Source adapters, REST/WebSocket/gRPC APIs, SDKs, cluster, plugins, production hardening.
