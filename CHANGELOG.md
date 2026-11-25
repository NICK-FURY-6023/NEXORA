# Changelog

All notable changes to NEXORA are documented here.

## [Unreleased]

### Added - community and documentation site

- Added `SECURITY.md` (security policy with a private vulnerability reporting path) and `CODE_OF_CONDUCT.md`.
- Added GitHub issue templates (`.github/ISSUE_TEMPLATE`) and a pull request template (`.github/PULL_REQUEST_TEMPLATE.md`).
- Added `book.toml` — the documentation site built from `docs/` with mdBook — plus CI `links` (markdown link check) and `site` (mdBook build and artifact upload) jobs. `docs/README.md` is the single reader-facing documentation index.

### Added - queue auto-advance in the playback task

- The pacing task now advances the queue when a track's packets run out: it
  pops the next queued track, emits `TrackEnded`, resolves and prepares it, and
  calls `play()` for it, which emits a new `TrackStarted` and resets the
  position to 0.
- The advance stays in the same task, so connected audio WebSocket clients keep
  their channel and pause/resume/seek/volume keep working across the track
  change (ADR-023).
- New `SessionManager::end_track()` marks a track as ended while the playback
  task stays attached to the session; a failed resolve or decode of the next
  track emits `TrackFailed`.
- Added ADR-023 for the queue auto-advance decision.

### Repository documentation map and ADR-021

- Added `docs/README.md` as the single reader-facing documentation index with
  repository map, ownership table, and quick start.
- Added ADR-021 for the documentation layout decision; marked the older
  `NEXORA_SYSTEM_DESIGN.md` draft as superseded.
- Updated contribution guide and root README to point to the docs map.

### Added - real-time local playback

- `play` resolves the track before mutating session state and starts a paced
  20 ms Opus playback task.
- Added binary audio WebSocket `/ws/v1/sessions/{id}/audio`, typed playback
  signals, `TrackEnded`/`TrackFailed`, and per-session bounded frame fanout.
- Verified with 82 passing tests and a live 43-packet Opus WebSocket smoke test.

### Added - SDK, deployment, cluster and plugin foundations

- Python and Node.js SDKs with tests; Java/Go/C#/PHP SDK sources.
- Docker/compose deployment artifacts and ADR-015..020.
- `nexora-cluster` registry/scheduler and `nexora-plugin-api` manifest/sandbox contract.
- Example monitoring configs and runtime metrics endpoints.

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
- `cargo test --workspace` (70 tests passed); Python 6 and Node 5 SDK tests; live REST, WebSocket, and gRPC smoke tests; end-to-end WAV to Ogg Opus conversion; short Criterion benchmark run

### Not included

- Source adapters, REST/WebSocket/gRPC APIs, SDKs, cluster, plugins, production hardening.
