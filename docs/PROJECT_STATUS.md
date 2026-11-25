# NEXORA Project Status

## Current milestone: Phase 5-7 foundations + deployment

Status: **implemented and verified** on 2026-11-23.

### Slice 1: real-time local playback (2026-11-22)

- `POST /api/v1/sessions/{id}/play` now resolves the track before mutating
  session state, prepares 20 ms Opus packets through the audio pipeline, and
  runs one paced playback task per session.
- `GET /ws/v1/sessions/{id}/audio` streams raw Opus packets as binary messages.
  The event WebSocket remains text-only.
- Pause/resume/seek/volume/stop send typed `PlaybackSignal`s to the playback
  task; volume is applied as PCM gain before encoding; `TrackEnded` and
  `TrackFailed` are real events.
- `nexora-session` owns a bounded per-session `PlaybackRegistry`; blocking
  locks were replaced with async locks so the runtime is never blocked.
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, and `cargo test --workspace` all exit 0 with
  82 passing tests. Live server smoke delivered 43 raw Opus packets in 1.3 s
  (first bytes `fc7f06de6f6e6d19`) and advanced session position to 900 ms.
- Deferred: queue auto-advance, packet-level pause/seek timing tests, Discord voice sender, and `/v4/decodetrack`.

### Slice 2: queue auto-advance (2026-11-23)

- The pacing task advances the queue itself: on end-of-packets it pops the next
  queued track (`pop_next_queued()`), emits `TrackEnded`, resolves and prepares
  the next track, and calls `play()` for it, which emits a new `TrackStarted`
  and resets the position to 0.
- The advance happens in the same task, so connected audio WebSocket clients
  keep their channel and pause/resume/seek/volume keep working across the
  track change (ADR-023).
- A failed resolve or decode of the next track emits `TrackFailed` and stops
  the session; an empty queue keeps the old terminal `finish()` behaviour.
- New `SessionManager::end_track()` marks a track as ended while the playback
  task stays attached to the session.
- Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, and `cargo test --workspace` all exit 0 with
  83 passing tests, including the new
  `playback_advances_to_the_next_queued_track` integration test.
- Live smoke: with a 3 s track playing and an 8 s track queued, the event
  WebSocket reported `track_started(toneA)` → `track_ended(reason=finished)` →
  `track_started(toneLong)` → `track_ended(reason=finished)`, the audio
  WebSocket received 256 Opus packets with no zero-length frame and no drop
  across the track change, and the session position restarted from 0 on the
  queued track.
- Deferred: packet-level pause/seek timing tests, the Lavalink `playerUpdate`
  event, Discord voice sender, and `/v4/decodetrack`.

### Implemented after Phase 4b

- Runtime API counters exposed through `/api/v1/metrics` and `/v4/stats`.
- Python SDK (6 tests) and Node.js SDK (5 tests); Java/Go/C#/PHP SDK sources are provided but their toolchains are unavailable on this host, so they are unverified by compilers.
- Docker/compose deployment artifacts, `.env.example`, and deployment docs.
- `nexora-cluster` worker registry/scheduler and `nexora-plugin-api` manifest/sandbox contract.
- ADR-015..020 and example monitoring configs.

Workspace total is now 153 passing Rust tests.

### Lavalink v4 parity (after the Phase 5-7 foundations)

- `docs/LAVALINK_ANALYSIS.md` records the v4 protocol behavior, REA evidence
  from the shipped `Lavalink.jar`, and deliberate non-goals.
- `/v4/loadtracks` now returns documented lowercase `loadType` values and the
  v4 track shape (`encoded`, `info`, `pluginInfo`, `userData`); `/v4/stats`
  uses `cpu.lavalinkLoad`. Three RED-GREEN contract tests cover the parity.
- `/v4/info` now returns the documented `version` object, numeric `buildTime`
  and `git.commitTime`, and real `sourceManagers` from the registry; `/v4`
  player responses carry `state.time`, `state.connected`, `state.ping` and
  `filters`; `POST /v4/sessions` returns 201. Two more contract tests cover it.
- Known compatibility boundary: `encoded` is an opaque NEXORA base64 JSON
  blob, not Lavalink's binary track encoding; `/v4/decodetrack` is absent.

### Lavalink v4 protocol core, WebSocket and session lifecycle

- `crates/nexora-protocol/src/v4/` owns every Lavalink v4 wire type: the op
  codes, the flat event envelope, the error envelope, the exception severities,
  the load results, the player and session objects, the filter chain, and the
  encoded-track codec. Every shape is read from the shipped
  `protocol-jvm-4.2.2.jar` rather than from the protocol documentation.
- The encoded-track codec is Lavaplayer's version-3 track-info frame, byte for
  byte: the reference blob in `docs/research/lavalink-source-architecture.md`
  §6.4 decodes to the documented fields and re-encodes to the same bytes.
- `services/nexora-server/src/lavalink.rs` serves the reference's route
  inventory: `/version`, `/v4/info`, `/v4/stats`, the three route-planner
  routes, `/v4/loadtracks`, `/v4/decodetrack`, `/v4/decodetracks`,
  `PATCH /v4/sessions/{id}`, the three player routes, and the
  `/v4/websocket` event socket. `POST /v4/sessions` is a NEXORA extension.
- The event socket emits the reference's flat frames: the `ready` frame first,
  then `playerUpdate`, `stats` and the event types. The reference's handshake
  rules are enforced before the upgrade — a missing or zero `User-Id` is a
  400, a wrong `Authorization` is a 401.
- `docs/COMPATIBILITY.md` records the row-by-row state and names the test that
  proves each row. The workspace total is now 153 passing Rust tests.

### Lavalink v4 events (M3)

- Every frame on `/v4/websocket` is flat: `op`, the `type` discriminator,
  `guildId` and the event's own fields are siblings, with no `data` wrapper.
- The reference's order for one track is reproduced exactly: the
  `TrackStartEvent`, then a `playerUpdate` immediately, then one every five
  seconds, then the `TrackEndEvent`. The update task is cancelled when the
  track ends, and it is skipped entirely while the player is paused.
- `TrackEndEvent` carries the reason the track stopped: `finished`,
  `loadFailed`, `stopped` or `replaced`.
- `TrackExceptionEvent` carries the exception's message, severity and root
  cause when a track cannot be decoded or resolved.
- The six NodeLink player events are served with NodeLink's payloads:
  `SeekEvent`, `PauseEvent`, `VolumeChangedEvent`, `FiltersChangedEvent`,
  `PlayerCreatedEvent` and `PlayerDestroyedEvent`.
- `docs/superpowers/specs/2026-11-25-nexora-m3-events-design.md` records the
  design and the bytecode evidence behind every claim.

### Implemented in Phase 4b

- `nexora-grpc` with a versioned `nexora.v1.SessionService` protobuf contract (tonic/prost, vendored protoc).
- gRPC RPCs: `CreateSession`, `GetSession`, `Play`, `Pause`, `Stop`, backed by the same in-memory `SessionManager` as the HTTP surface.
- `nexora-server` starts the gRPC listener alongside REST/WebSocket; `--grpc-port` (default 9090) configures it.
- Live gRPC smoke: `session=nx_session_1 created_status=created live_status=created`.

Phase 4b adds 1 integration test and 1 example client; the workspace total is now 63 passing tests.

### Implemented in Phase 4a

- `nexora-session`: in-memory session lifecycle with queue, volume, seek, pause/resume/stop and broadcast event envelopes.
- `nexora-server`: Axum server with NEXORA `/api/v1` REST endpoints, WebSocket events at `/ws/v1/sessions/{id}`, and a Lavalink-compatible `/v4` surface (`info`, `loadtracks`, sessions, player GET/PATCH, WebSocket events).
- Bearer-token authorization via `--password` or `NEXORA_SERVER_PASSWORD`; unset password keeps local development open.
- Consistent `{ error: { code, message } }` API errors.
- Local-file audio streaming endpoint that decodes/resamples/encodes to Ogg Opus through the Phase 2 pipeline.

Live smoke: `/health` 200, `/v4/info` 200, NEXORA session create 201, play 200, Lavalink session create 200, and a real WebSocket `session_ready` event after a volume change.

Phase 4a adds 5 tests; the workspace total is now 62 passing tests.

### Implemented in Phase 3

- `nexora-source-api`: `AudioSource` trait, auth context, track metadata/reference, resolved source, source health, normalized `SourceError`.
- `nexora-source-framework`: source registry, timeout wrapper, cancellation token, candidate ranking, health aggregation.
- Adapters: local media (directory-scoped, path-traversal blocked), public radio (configured stations), authorized HTTP stream (scope + URL validation).
- SSRF-safe URL validation for http(s) targets; private/loopback/link-local/localhost targets are rejected without DNS resolution.

Phase 3 adds 9 tests; the workspace total is now 57 passing tests.

### Implemented in Phase 2

- `nexora-audio-decode`: Symphonia local-media decoding to interleaved f32 PCM.
- `nexora-audio-codec`: PCM16 WAV writer, pure-Rust Opus encoder, Ogg Opus container writer.
- `nexora-audio-pipeline`: Rubato resampling, clipping detection, bounded queue, channel remap, `process_file` orchestration.
- `services/nexora-audio-engine`: CLI with frames, elapsed time, and real-time-factor reporting.
- Criterion benchmark and regression test for Opus partial final-frame padding.

### Previously completed (Phase 1)

- Rust workspace pinned to Rust 1.100.0 and edition 2024.
- `nexora-error`: structured error codes and redaction-safe error messages.
- `nexora-common`: validated IDs, service names, API version, timestamp type.
- `nexora-audio-types`: audio frame/format/codec types and a reusable frame-pool contract. Types only; no decoding.
- `nexora-protocol`: versioned session DTOs, event envelope, command enum.
- `nexora-config`: layered configuration (defaults, optional TOML, environment overrides) with validation.
- `nexora-telemetry`: idempotent tracing initialization and metric-name constants.
- `nexora-service`: service trait, health report, shutdown coordination.
- `services/nexora-gateway`: skeleton binary that loads config, starts tracing, waits for Ctrl-C, and shuts down gracefully. No HTTP listener yet.
- GitHub Actions CI baseline: formatting, clippy, and workspace tests on Ubuntu and Windows.

### Verification evidence

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three checks passed locally. 42 tests passed across the workspace.

End-to-end: 1 s 44.1 kHz mono WAV -> 48 kHz Ogg Opus. Input frames 44,100; output frames 48,001; 51 packets; 111 ms elapsed; 8.98x real-time factor; 8,519-byte Ogg file with valid OpusHead/OpusTags.

Criterion benchmark `resample_44100_to_48000_8192_frames` (release, Rust 1.100.0, Windows GNU, 100 ms warm-up, 200 ms measurement, 10 samples): `time: [1.5484 ms 3.4276 ms 7.2193 ms]`, 2 high-severe outliers. This is a short noisy run, not a production performance claim.

### Not implemented yet

- Phase 3: source adapters, resolver, health checks, SSRF-safe URL handling.
- Phase 4: REST, WebSocket events, gRPC, session lifecycle.
- Phase 5-8: SDKs, distributed cluster, plugins, production hardening.

### Next milestone

Remaining work: full Lavalink client conformance, WASM plugin runtime, durable session store, and production security review.
Next milestone after that: Phase 5 SDKs.

### Status policy

Mark a feature implemented only when code, tests, and documentation exist. Publish performance numbers only with reproducible methodology and hardware/software details.
