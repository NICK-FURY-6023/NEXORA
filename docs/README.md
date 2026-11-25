# NEXORA Documentation

Start here. This is the navigation page for users, operators, contributors, and
AI agents. It maps questions to the one authoritative document that answers
them.

## Start by role

| I want to… | Read |
|---|---|
| Understand what NEXORA is | [Root README](../README.md) → [System design](SYSTEM_DESIGN.md) |
| Run the server locally | [Quick start](#quick-start) → [Deployment](DEPLOYMENT.md) |
| Call the API | [API contracts](API.md) → [Wire contract](PROTOCOL.md) |
| Check Lavalink/NodeLink compatibility | [Compatibility matrix](COMPATIBILITY.md) |
| Read the documentation site | [mdBook site source](../book.toml) — built from this `docs/` tree; run `mdbook build` to render locally |
| Migrate from Lavalink | [Lavalink analysis](LAVALINK_ANALYSIS.md) → [v4 surface](API.md) |
| Understand implemented architecture | [Implemented structure](ARCHITECTURE.md) |
| Understand the full-parity rebuild plan | [Full-parity design](superpowers/specs/2026-11-24-nexora-full-parity-design.md) → [M3 events design](superpowers/specs/2026-11-25-nexora-m3-events-design.md) |
| Follow a milestone plan | [M0 protocol core plan](superpowers/plans/2026-11-24-nexora-m0-protocol-core.md) → [M1 v4 websocket plan](superpowers/plans/2026-11-24-nexora-m1-v4-websocket.md) → [M2 session lifecycle plan](superpowers/plans/2026-11-24-nexora-m2-session-lifecycle.md) → [community and site plan](superpowers/plans/2026-11-24-nexora-community-and-site.md) |
| Understand why decisions were made | [ADR index](adr/README.md) → [ADR-022 playback pipeline](adr/ADR-022-playback-pipeline-architecture.md) → [ADR-023 queue auto-advance](adr/ADR-023-queue-auto-advance.md) |
| Understand why decisions were made | [ADR index](adr/README.md) |
| Develop or review code | [Development](DEVELOPMENT.md) → [Detailed contributing guide](CONTRIBUTING.md) → [Root checklist](../CONTRIBUTING.md) |
| Check current truth/status | [Project status](PROJECT_STATUS.md) → [Changelog](../CHANGELOG.md) → [Today's schedule](TODAY_SCHEDULE.md) |
| Review security posture | [Security review](SECURITY_REVIEW.md) |
| Study reference evidence | [Research index](#research-and-reference-evidence) |
| See a bot-shaped example | [Bot PoC](BOT_POC.md) and [example script](../examples/bot_poc.py) |

## Quick start

```bash
cargo build -p nexora-server
mkdir -p data/media
./target/debug/nexora-server --host 127.0.0.1 --port 8080 --grpc-port 9090 \
  --media-root ./data/media
```

In another shell:

```bash
curl -fsS http://127.0.0.1:8080/health
python3 examples/bot_poc.py
```

`set NEXORA_SERVER_PASSWORD` for anything beyond local development. With no
password, authorization is disabled for local use only. See
[Deployment](DEPLOYMENT.md) for Docker and production notes.

## Repository map

```text
.
├── Cargo.toml / Cargo.lock       # Rust workspace, pinned dependencies
├── crates/                       # Reusable Rust libraries
│   ├── nexora-audio-*            # types, decode, codec, pipeline
│   ├── nexora-source-api         # AudioSource, auth, track contracts
│   ├── nexora-source-framework   # registry, ranking, local/radio/HTTP adapters
│   ├── nexora-session            # sessions, queue, playback control, Opus fanout
│   ├── nexora-protocol           # versioned DTOs and event envelope
│   ├── nexora-grpc               # tonic adapter over SessionManager
│   └── nexora-config/error/...   # shared foundation
├── services/                     # Deployable binaries
│   ├── nexora-server             # REST, event WS, audio WS, gRPC listener
│   ├── nexora-gateway            # skeleton control-plane service
│   └── nexora-audio-engine       # standalone batch transcode CLI
├── sdks/                         # Python, Node.js, Java, Go, C#, PHP clients
├── deploy/                       # Docker Compose, env example, monitoring
├── examples/                     # Runnable proof-of-concept clients
├── assets/                       # Images and infographics
└── docs/                         # This documentation tree
```

Detailed ownership is in [ADR-021](adr/ADR-021-repository-documentation-layout.md)
and in the table below.

## Where does a change belong?

| Change | Responsible place | Proof |
|---|---|---|
| New REST or WS route | `services/nexora-server/src/lib.rs` | `services/nexora-server/tests/http.rs` |
| Session/queue/playback invariant | `crates/nexora-session` | `crates/nexora-session/tests/session.rs` |
| Source adapter or SSRF rule | `crates/nexora-source-framework` | `crates/nexora-source-framework/tests/framework.rs` |
| Decode/resample/encode behavior | `crates/nexora-audio-*` | crate tests + `cargo bench` where relevant |
| Public DTO shape | `crates/nexora-protocol` | round-trip tests + [API.md](API.md) |
| Lavalink wire compatibility | `/v4` handler in `nexora-server` | contract test + [LAVALINK_ANALYSIS.md](LAVALINK_ANALYSIS.md) |
| Deployment/monitoring | `deploy/` | [DEPLOYMENT.md](DEPLOYMENT.md) |
| Architecture decision | `docs/adr/` | new ADR, never rewrite old ADRs |

## Documentation authority

| Document | Authority |
|---|---|
| [README.md](../README.md) | Product vision and long-term architecture |
| [SYSTEM_DESIGN.md](SYSTEM_DESIGN.md) | Implemented system design and invariants |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Current crate/service structure |
| [API.md](API.md) | Native `/api/v1`, audio WS, and `/v4` API contracts |
| [PROTOCOL.md](PROTOCOL.md) | Implemented wire contract, auth, and error envelope |
| [PROJECT_STATUS.md](PROJECT_STATUS.md) | What is verified right now |
| [LAVALINK_ANALYSIS.md](LAVALINK_ANALYSIS.md) | Lavalink protocol reference and boundaries |
| [ADRs](adr/README.md) | Why decisions were made |

`docs/NEXORA_SYSTEM_DESIGN.md` is an early draft and is superseded by
[SYSTEM_DESIGN.md](SYSTEM_DESIGN.md). It stays only as historical research.

## Research and reference evidence

- [Lavalink analysis](LAVALINK_ANALYSIS.md) — short protocol reference.
- [Lavalink deep dive](LAVALINK_DEEP_DIVE.md) — observed/inferred internals.
- [REA static analysis](research/lavalink-rea-static-analysis.md) — shipped JAR evidence.
- [Source architecture](research/lavalink-source-architecture.md) — line-cited source-level behavior spec.
- [System design](SYSTEM_DESIGN.md) — what NEXORA actually implements.

Official contract: <https://lavalink.dev/api/rest.html>,
<https://lavalink.dev/api/websocket.html>, <https://lavalink.dev/changelog/v4>.

## Verification commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Current verified baseline: 82 tests passing, 0 failures. See
[PROJECT_STATUS.md](PROJECT_STATUS.md) for the live runtime proof.
