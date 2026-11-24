# NEXORA full-parity rebuild — design

Date: 2026-11-24. Status: proposed, awaiting approval.

## Intent and why

NEXORA must become a ground-up Rust implementation of the Lavalink v4 audio
node surface, plus the NodeLink behaviours that are worth having. Not a
wrapper: no Java runtime, no `Lavalink.jar` dependency, no NodeLink
subprocess, no copied source. Behaviour is studied, then reimplemented
originally in Rust.

The "why" is a Rust-native audio node that existing Lavalink clients can talk
to without a JVM in the deployment, with a plugin/source story that does not
inherit the Java plugin ecosystem.

## Goals

- Lavalink v4 REST and WebSocket compatibility, proven by a compatibility
  matrix and real-client tests, not by claims.
- The NodeLink additions that cost nothing extra: `loadlyrics`,
  `loadchapters`, `connection`, `trackstream`, the extra filters, the extra
  stats detail.
- Discord voice delivery with DAVE E2EE, verified against the current
  protocol rather than assumed.
- A modular Rust workspace where protocol, server, player, sources, audio,
  voice, config and observability are separable and testable.
- Documentation that ships in the same milestone as the code it describes.

## Non-goals

- Running Lavalink's Java plugin jars. A pure-Rust server cannot load a
  `URLClassLoader` plugin; the WASM plugin runtime is the substitute and is a
  later milestone.
- Commercial music providers (Spotify, Apple Music, Tidal, Deezer and their
  token/cookie requirements). This stays a non-goal; sources are local files,
  HTTP and radio, plus a source-manager framework to extend.
- Claiming "100% drop-in" before the matrix passes. The wording is "target",
  with a per-row status.
- Porting Lavalink's or NodeLink's source code.

## Anti-goals

- A single huge rewrite commit that replaces the workspace at once.
- Behaviour copied from strings or memory instead of from observed evidence.
- Any slice that lands without its proof (fmt, clippy, tests, parity diff).

## Constraints

- Rust stable, edition 2024, pinned dependencies, `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test
  --workspace` green before any milestone closes.
- No `unwrap`, `expect` or `panic!` in production paths that can fail at
  runtime; errors flow through `nexora-error`.
- Third-party crates are allowed (Tokio, Serde, Rubato, Symphonia are already
  used). The server, protocol, session and player logic are ours.
- Dependencies respect their licences; attribution is recorded.
- Large downloads and installs go to the E drive.
- Documentation layout follows ADR-021: this design doc is linked from
  `docs/README.md`, and it is a working document, not an ADR.

## Approaches

**A. Map the target architecture onto the existing workspace (chosen).**

The target layout in the brief (`crates/protocol`, `crates/server`,
`crates/player`, `crates/sources`, `crates/audio`, `crates/voice`,
`crates/config`, `crates/observability`) maps one-to-one onto crates that
mostly exist already, under the project's `nexora-` naming and the
`crates/` + `services/` split:

| Target | NEXORA today | Action |
|---|---|---|
| `crates/protocol` | `crates/nexora-protocol` | keep; add v4 op codes, version negotiation |
| `crates/server` | `services/nexora-server` | keep; add missing v4 routes and `WS /v4/websocket` |
| `crates/player` | `crates/nexora-session` + the pacing task in the server | keep; split the pacing task into `crates/nexora-player` when the state machine grows |
| `crates/sources` | `crates/nexora-source-api`, `crates/nexora-source-framework` | keep |
| `crates/audio` | `crates/nexora-audio-{types,decode,codec,pipeline}` | keep; add the filter chain |
| `crates/voice` | — | **new** `crates/nexora-voice` |
| `crates/config` | `crates/nexora-config` | keep |
| `crates/observability` | `crates/nexora-telemetry` | keep; extend metrics |
| `tests/compat` | — | **new** `services/nexora-server/tests/compat/` — cargo has no root test target, so the harness lives with the package it exercises |

Why: the names, ownership table and ADR-021 documentation layout already
describe this structure. Renaming 15 crates would touch every import, the
ownership table, the ADRs and the docs map for no behaviour gain.

**B. Restructure to the generic crate names (rejected).** Pure churn: every
`use nexora_*` becomes `use protocol::…`, the ownership table and ADR index
need rewriting, and the git history of 83 passing tests becomes harder to read
for zero behavioural gain. If the generic names are wanted later, it is a
mechanical follow-up commit.

**C. Port Lavalink's Java implementation (forbidden by the brief).** It would
inherit the JVM the project exists to avoid, and it is not a licence-clean
path.

## Milestones and exit proof

Each milestone is one rust-anything parity unit where the original's
behaviour is recorded and the Rust output is diffed against it; the unit
counts as done only when `gate` passes.

| # | Milestone | Exit proof |
|---|---|---|
| M0 | Protocol core: v4 op codes, error envelope, version negotiation, track encoding | protocol contract tests; encoding round-trip fixtures |
| M1 | Server skeleton: `/health`, `/v4/info`, `/version`, auth, `/v4/websocket` handshake | real Lavalink client connects; matrix row green |
| M2 | Session lifecycle: create, `loadTracks`, play, pause, stop, seek, volume, `PATCH /v4/sessions/{id}` | real client play/pause/resume; matrix rows green |
| M3 | Events: `playerUpdate`, `TrackStart/End/Exception`, queue events | event ordering tests against recorded Lavalink behaviour |
| M4 | Filters: Lavalink's ten built-ins (`volume`, `equalizer`, `karaoke`, `timescale`, `tremolo`, `vibrato`, `distortion`, `rotation`, `channelMix`, `lowPass`) plus the NodeLink extras, applied to PCM before encoding | filter unit tests + parity diff |
| M5 | Discord voice: gateway voice update, UDP, encryption, DAVE E2EE | live voice test against a disposable voice server or a documented harness |
| M6 | Session resume: reconnect restores player state and position | reconnect/resume test |
| M7 | Missing v4 surface: `/v4/decodetrack`, `/v4/decodetracks`, `/v4/sessions/{id}/players[/guildId]`, route-planner free endpoints | matrix rows green |
| M8 | NodeLink extras: `loadlyrics`, `loadchapters`, `connection`, `trackstream`, detailed stats | matrix rows green; sources for lyrics/chapters documented |
| M9 | Source framework: local, HTTP, radio, plus a source-manager extension point | source contract tests; SSRF rules re-verified |
| M10 | WASM plugin runtime (contract exists; execution deferred until here) | sandbox tests; plugin API doc |
| M11 | Compatibility matrix, docs, benchmarks, release | every matrix row either green or explicitly not-targeted, with a reason |

## Compatibility strategy

- The matrix lives in the docs map with one row per Lavalink v4 route, event,
  filter and NodeLink extra: status, evidence, test name.
- Parity is proved by recording the original's behaviour and diffing the Rust
  output. Where the original cannot be run on this host, the row states the
  evidence type instead of claiming parity.
- `/v4/info` keeps NEXORA's own identity but reports the compatibility
  surface truthfully.

## Skill workflow gates

| Stage | Skill | Gate |
|---|---|---|
| Behaviour analysis | REA | every behavioural claim cites an evidence id |
| Study to recreate | craft-anything | brief, plan, findings and handoff kept current; findings become spec rows |
| Implementation | rust-anything | one parity unit at a time; `record` → `diff` → `gate` |
| Review and hardening | ecc | rust review and code review per milestone; findings fixed before the milestone closes |
| Process | superpowers | design approved before building; plans written before multi-step work; verification before claiming done |

## Quality bar

- fmt, clippy with warnings as errors, and the workspace test suite green at
  every milestone close.
- No `unwrap`/`expect`/`panic!` in production paths; fuzzing of the protocol
  parser and the track-encoding path.
- `tracing` instrumentation on the request, session and pacing paths.
- Security review of the auth boundary, the SSRF rules and the path handling
  before M11.

## Documentation deliverables

README (features, non-goals, quickstart), Docker and config examples, the
protocol compatibility matrix, architecture, a Lavalink migration guide, client
examples for the existing SDKs, benchmark notes, the roadmap, and licence
attribution.

## Open items and unknowns

- **DAVE E2EE**: the brief asks whether DAVE is mandatory for current Discord
  voice. This is a fact to verify from current protocol evidence during M5,
  not an assumption to encode now.
- **NodeLink extra events**: the differences page states NodeLink emits extra
  events but does not list them; the NodeLink source is the authority.
- **Lavalink's plugin-only capabilities** (YouTube OAuth, SponsorBlock,
  LavaSearch) are not reproduced; the matrix will mark them not-targeted.

## What is left for the builder

- Build the gap table from the REA evidence (Lavalink jar + NodeLink source)
  and the current NEXORA surface, one row per behaviour, with evidence.
- Turn the gap table into the ordered unit list for M0-M11.
- Write the implementation plan per unit.
