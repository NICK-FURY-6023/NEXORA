# NEXORA Complete System Design

Status: design baseline plus implemented slice 1, 2026-11-22. This document
records observed code behavior, the invariants to preserve, the complete system
design, and the first working vertical slice. Later slices are explicitly
planned, not claimed as implemented.

Evidence used: `README.md`, `docs/MASTER_AI_INSTRUCTIONS.md`,
`docs/LAVALINK_ANALYSIS.md`, official Lavalink v4 REST/WebSocket docs, the
current Rust sources cited below, and a fresh `cargo test --workspace` run
(exit 0, 82 tests passed).

## 1. Product intent

NEXORA is a Rust, standalone audio node: one process, no JVM, no hosted Lavalink
node, no scraper or third-party media service required to run. It exposes:

- a native, language-independent `/api/v1` contract;
- a Lavalink-compatible `/v4` adapter for client migration;
- local/authorized stream sources behind one `AudioSource` abstraction;
- an audio pipeline that decodes, resamples, remaps channels, and emits 20 ms
  Opus packets;
- session lifecycle, queue, events, metrics, REST, WebSocket, and gRPC.

"Independent" means independent of third-party **services**. Compile-time Rust
crates (tokio, axum, symphonia, opus-rs, rubato) are a recorded tradeoff; a
zero-crate rewrite of HTTP, TLS, codecs, and containers is out of scope for
this product. See `docs/LAVALINK_ANALYSIS.md:80-88`.

## 2. What Lavalink actually does

Lavalink is a standalone audio-sending node, mostly Kotlin on the JVM. A bot
sends commands over REST; the node resolves media, decodes it, produces Opus,
and sends frames to a Discord voice server. Audio never flows through the bot's
gateway process. Official v4 contract facts:

- All client commands are REST; `/v4/websocket` is server-push only. Client
  commands over WebSocket were removed in v4. (`https://lavalink.dev/changelog/v4`)
- `GET /v4/loadtracks?identifier=...` returns `loadType` of `track`, `playlist`,
  `search`, `empty`, or `error`; tracks carry `encoded`, `info`, `pluginInfo`,
  `userData`. (`https://lavalink.dev/api/rest`)
- Player updates carry track, voice state (token, endpoint, session id,
  channel id), pause, volume, and filters. (`https://lavalink.dev/api/rest`)
- WebSocket ops are `ready`, `playerUpdate`, `stats`, and `event`; stats use
  `cpu.lavalinkLoad`; frame stats expect 3000 frames/minute/player, one per
  20 ms, with `deficit` measuring missed frames.
  (`https://lavalink.dev/api/websocket`)
- v4.2.0 added DAVE end-to-end encrypted voice; v4.2.2 uses koe 3.0.0-pre6 and
  libdave 0.1.3. (`https://lavalink.dev/changelog/v4`)
- REA evidence for the shipped `Lavalink.jar` (v4.2.2, SHA-256
  `8cb801e591072c36...`) is already recorded in `docs/LAVALINK_ANALYSIS.md:41-56`.
  The JAR inventory is an observation; internal control flow from class names
  remains an inference.

The idea worth copying is the boundary: control plane separate from audio
plane. The JVM/plugin/classloading, route planner, rate-limit evasion, and
proprietary source extraction are deliberate non-goals
(`docs/LAVALINK_ANALYSIS.md:72-78`).

## 3. What exists today (observed)

| Layer | Current reality | Evidence |
|---|---|---|
| Workspace | 19 members, Rust 1.100, edition 2024, pinned deps | `Cargo.toml:1-53`, `rust-toolchain.toml` |
| Control plane | Axum router with REST/WS routes plus gRPC listener | `services/nexora-server/src/lib.rs`, `src/main.rs` |
| Session state | In-memory `SessionManager`: status, track, position, volume, queue, output format, playback signals, broadcast events | `crates/nexora-session/src/lib.rs` |
| Source framework | Registry, ranking, 3 s search timeout, cancellation, local-media/radio/HTTP adapters, basic SSRF checks | `crates/nexora-source-framework/src/lib.rs` |
| Audio pipeline | Whole-file decode to f32, Rubato resample, channel remap, clipping flag, WAV/Ogg Opus writers, 20 ms packet preparation, bounded PCM queue | `crates/nexora-audio-decode/src/lib.rs`, `crates/nexora-audio-pipeline/src/lib.rs`, `crates/nexora-audio-codec/src/lib.rs` |
| `/api/v1/.../stream` | Resolves current local track, batch-transcodes the entire file into a temp Ogg file, returns bytes, deletes temp file | `services/nexora-server/src/lib.rs` |
| Live playback | `play` resolves source, starts one paced task, emits raw Opus to `/ws/v1/sessions/{id}/audio`; controls use typed signals | `crates/nexora-session/src/lib.rs`, `services/nexora-server/src/lib.rs` |
| `/v4` adapter | `info`, `stats`, `loadtracks`, `sessions`, `player`, WS events; real source IDs and honest `connected=false`, `ping=-1` | `services/nexora-server/src/lib.rs` |
| Auth | Bearer token when password set, open when unset; REST and both WebSockets share it | `services/nexora-server/src/lib.rs` |
| Tests | 82 passing in workspace; live audio WS proof recorded below | fresh `cargo test --workspace` + live smoke |

Slice 1 closed the original playback gap: `play` now resolves before mutating
session state, produces paced 20 ms Opus packets, advances position, and emits
`TrackEnded` or `TrackFailed`. Queue auto-advance from `TrackEnded` into the next
queue item is implemented in the pacing task (`pop_next_queued()` exposes FIFO
order; the task pops the next track and calls `play()` for it — ADR-023).
NEXORA is still not a drop-in Lavalink replacement because no Discord voice
sender exists yet.

## 4. Invariants to preserve

1. **20 ms Opus framing.** One packet per 20 ms, monotonic sequence and
   timestamp. This is the Lavalink frame-accounting contract and our pacing
   base (`docs/LAVALINK_ANALYSIS.md:38-39,60`).
2. **Audio never enters the gateway/control path as a control message.**
   Standalone mode may carry audio in a dedicated delivery channel, but the
   event WebSocket stays control-only (`docs/ARCHITECTURE.md:68-75`).
3. **Source resolution before state change.** `play` must not report `playing`
   unless the track can be resolved. Failure emits `TrackFailed`, not a silent
   success.
4. **Bounded buffers and explicit backpressure.** Reuse
   `BoundedAudioQueue` semantics (`crates/nexora-audio-pipeline/src/lib.rs:157-198`,
   `docs/adr/ADR-020-bounded-audio-queue.md`). Drop or reject rather than grow
   memory without limit.
5. **Session ownership.** One active playback task per session. A new `play`
   signals `Stop` to the old task before attaching a new one. Session-scoped
   events only; no cross-session leaks (`crates/nexora-session/src/lib.rs:47-49,230-246`).
6. **Typed errors and stable error envelope.** Trust-boundary errors map to
   `{ error: { code, message } }` without internal details
   (`services/nexora-server/src/lib.rs:78-161`).
7. **Authorization at the trust boundary.** Bearer token, local root containment,
   SSRF validation, no secrets in code/logs (`crates/nexora-source-framework/src/lib.rs:205-249,317-341`,
   `docs/ARCHITECTURE.md:77-83`).
8. **Compatibility is an adapter.** `/v4` stays a translation layer; `/api/v1`
   remains the durable contract (`docs/adr/ADR-016-lavalink-compatible-surface.md`,
   `docs/LAVALINK_ANALYSIS.md:77-78`).
9. **In-process state is acceptable for the first milestone; durability is a
   later phase.** (`docs/adr/ADR-017-in-process-session-state.md`)

## 5. Target architecture

```text
Bot / app / SDK
  | REST commands + event WS + audio WS
  v
nexora-server (standalone control + delivery host)
  |--- /api/v1 native contract
  |--- /v4 compatibility adapter
  |--- SessionManager (session/queue/status/volume/position)
  |--- SourceRegistry (resolve, search, scope checks)
  |--- PlaybackRegistry (per-session bounded Opus frame fanout)
  v
audio pipeline (data plane)
  decode -> resample -> channel remap -> volume -> 20 ms Opus packets
  |
  +--> dedicated audio WS consumer (universal clients)
  +--> future Discord voice sender (Lavalink drop-in path)
```

Crate ownership:

| Crate | Owns | Must not own |
|---|---|---|
| `nexora-protocol` | Versioned DTOs, event envelope, command enum | Transport, audio |
| `nexora-session` | Session lifecycle, queue, position, volume, playback control signals | Decoding, source lookup, network |
| `nexora-source-api` / `nexora-source-framework` | Source contracts, registry, ranking, timeout, path/SSRF safety | HTTP handlers, audio encoding |
| `nexora-audio-decode` | Local file probe/decode to interleaved f32 | Session state |
| `nexora-audio-pipeline` | Resample/remap/clipping, packet preparation, bounded queues | Network, sessions |
| `nexora-audio-codec` | Opus packet encoding, WAV/Ogg writers | Protocol shapes |
| `nexora-server` | Wiring, REST/WS handlers, auth, metrics, playback task, delivery channels | New business invariants |
| `nexora-grpc` | Thin RPC adapter over the same `SessionManager` | A second session model |

## 6. Simplest complete system (slice 1, implemented)

Scope: a standalone server that can genuinely play a local file into a
subscribed client at real time, with working control and events, no external
service.

In scope:

- `POST /api/v1/sessions`, `/play`, `/pause`, `/resume`, `/stop`, `/seek`,
  `/volume`, `/queue`;
- `GET /api/v1/search`, `/api/v1/sessions/{id}`, `/api/v1/metrics`;
- `GET /ws/v1/sessions/{id}` for text events;
- `GET /ws/v1/sessions/{id}/audio` for binary Opus packets;
- local-media source only for playback (radio/HTTP streams remain search-only
  until an HTTP client is added);
- 48 kHz stereo Opus output, 20 ms packets, volume as PCM gain, seek by packet
  index, pause/resume/stop as playback control;
- `TrackStarted`, `TrackEnded`, `TrackFailed`, `PlaybackPaused`,
  `PlaybackResumed`, `PlaybackStopped`, `QueueUpdated`, `TrackPosition`;
- `/v4` remains honest: no voice connection, so `connected=false`, `ping=-1`.

Out of scope for this slice:

- Discord gateway/voice UDP/DAVE, `/v4/decodetrack`, filters, plugins, cluster,
  durable sessions, YouTube/Spotify providers, rate-limit evasion.

Runtime flow:

1. Client searches via `SourceRegistry` and gets a track id.
2. `POST /play` resolves the id to a `ResolvedSource::LocalFile` first. If no
   file is found, return 404/422 and emit `TrackFailed`; do not set `playing`.
3. Server reads current volume, asks audio pipeline for 20 ms Opus packets,
   creates a per-session bounded broadcast channel, attaches a playback control
   receiver to the session, and spawns one pacing task.
4. The pacing task sends at most one packet per 20 ms tick, updates session
   position, counts streamed bytes, and reacts to pause/resume/seek/volume/stop.
   A volume change restarts packet preparation with the new PCM gain at the
   current position; this keeps gain correct without touching encoded packets.
5. On end-of-packets it pops the FIFO queue. With an empty queue it emits
   `TrackEnded` and stops the session. With a queued track it emits
   `TrackEnded`, clears the current track, resolves and prepares the next
   track, and calls `play()` for it — the same pacing task continues and emits
   a new `TrackStarted`, so connected audio subscribers keep their channel
   (ADR-023).
6. The audio WebSocket sends one Opus packet per binary message in order. The
   event WebSocket remains text-only.

Key types to add:

```text
nexora-audio-pipeline::OpusFrame { sequence, timestamp_ms, duration_ms, payload }
nexora-audio-pipeline::encode_file_packets(path, rate, channels, volume) -> Vec<Vec<u8>>
nexora-session::PlaybackSignal { Pause, Resume, Stop, Seek(u64), SetVolume(u8) }
nexora-server::PlaybackRegistry: HashMap<SessionId, broadcast::Sender<OpusFrame>>
```

`process_file` should share one PCM preparation helper with
`encode_file_packets` so batch transcode and live playback cannot drift.

## 7. API and wire contracts

Native delivery channel:

- Path: `/ws/v1/sessions/{session_id}/audio`
- Auth: same Bearer token as the REST API.
- Message: binary, one raw Opus packet per message; no JSON wrapper.
- Order: WebSocket order equals packet sequence; nominal rate 50 packets/s.
- Gap handling: if a client lags and `broadcast::error::RecvError::Lagged(n)` is
  returned, the consumer skips `n` messages and continues; it never blocks the
  producer.

Event channel remains `EventEnvelope` JSON text at `/ws/v1/sessions/{id}`.

`/v4` adapter rules:

- Keep lowercase `loadType`, v4 track shape, `cpu.lavalinkLoad`, 201 session
  create, numeric times, real source manager IDs.
- Add voice state and frame stats only with a real voice sender. Do not
  advertise capabilities the engine lacks (`docs/API.md:11`,
  `docs/LAVALINK_ANALYSIS.md:113-117`).

## 8. Security design

- Bearer check on every REST/WS route except `/health`, `/api/v1/info`,
  `/v4/info` as today; adding new routes must use the same `authorize` helper.
- Local media stays under the configured root; traversal, absolute paths, and
  parent components remain rejected (`crates/nexora-source-framework/src/lib.rs:325-340`).
- `ResolvedSource::Stream` is not fetched in this slice, so no new SSRF surface.
  When a stream client is added, reuse `validate_public_http_url`, resolve all
  DNS records, reject private/reserved IPs, pin the verified IP, and disable
  redirects to close the rebinding gap.
- No audio payload, token, or path is logged. Metrics count bytes only.
- Bounded broadcast channels prevent a slow client from growing memory.
- Volume is clamped to the existing 0..150 range
  (`crates/nexora-session/src/lib.rs:170-173`); gain above 100 may clip and is
  documented as such.

## 9. Verification plan

For each slice, in order:

1. RED contract test for the new wire/behavior invariant.
2. Implement in the owning crate only.
3. `cargo fmt --all -- --check`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `cargo test --workspace`
6. Live server smoke: start `nexora-server` with a temporary media root,
   create a session, play a generated WAV, connect the audio WS, and record real
   packet count/timing; stop the process afterwards.
7. Update docs with exact evidence and commit the slice.

Acceptance evidence for slice 1 (fresh run, 2026-11-22):

- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0.
- `cargo test --workspace`: exit 0, 82 tests passed, 0 failed.
- Live smoke: `nexora-server --host 127.0.0.1 --port 18090 --grpc-port 18091
  --media-root <temp>`; `/health` 200, session create 201, `/play` 200, audio
  WebSocket connected, 43 raw Opus packets received in 1.3 s, first bytes
  `fc7f06de6f6e6d19`, packet sizes started `289,167,166,160,159`, session state
  reached `position_ms=900`; process stopped afterwards.

Slice 1 does not prove every acceptance condition above: pause/resume/seek/stop
were verified by session signal and protocol tests, not by packet-level live
timing. Those measurements belong to slice 2; queue auto-advance is done
(ADR-023).

## 10. Roadmap

| Slice | Deliverable | Exit proof |
|---|---|---|
| 0 (done) | Workspace, protocol, config, errors, audio crates, source framework, REST/WS/gRPC, SDKs, deployment, partial v4 | 75 tests; REA + docs recorded |
| 1 (done) | Real-time local-file playback + dedicated binary Opus WS + working controls | fmt/clippy; tests green; live Opus packets |
| 2 (in progress) | Queue auto-advance (done, ADR-023) + `TrackEnded` reason + `playerUpdate` event | Contract tests for end reasons and position updates |
| 3 | Discord voice sender: voice update REST, voice gateway v8, UDP, encryption, DAVE boundary | Live test against a disposable voice server or documented harness |
| 4 | `/v4/decodetrack`, filters, resumable sessions, frame stats, real CPU/memory metrics | Lavalink client compatibility tests |
| 5 | Durable sessions, clustering, WASM plugins, production hardening | Deployment and chaos/conformance tests |

Slice 2 depends on slice 1 because auto-advance needs a real end-of-track signal.
Slice 3 depends on slice 1 because the voice sender consumes the same Opus frame
fanout. Later slices stay behind explicit evidence gates.

## 11. Current unresolved boundaries

- Queue auto-advance is implemented in the pacing task (ADR-023): it pops the
  FIFO queue, emits `TrackEnded`, and starts the next track in place. The
  remaining slice-2 work is the Lavalink `playerUpdate` event and
  packet-level pause/seek timing tests; one pacing task still serves one
  session, so many concurrent sessions need a worker pool.
- No real voice delivery means NEXORA is not yet a drop-in replacement for a
  Discord music bot; slice 3 is required for that claim.
- `encoded` is an opaque NEXORA base64 JSON blob, not Lavalink's binary track
  encoding. `/v4/decodetrack` is absent by design for now.
- Session state is in-memory and lost on restart (ADR-017).
- Whole-file packet preparation is acceptable for the first slice, bounded by a
  documented local-file size policy; streaming decode from a reader is the next
  optimization, not a correctness requirement.
- Background subagents for this analysis hit provider HTTP 429. Their output was
  discarded; this document is based on direct source/official-doc evidence.
