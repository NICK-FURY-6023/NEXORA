# NEXORA vs Lavalink vs NodeLink

This is an evidence-based comparison, not a feature spreadsheet written from
marketing pages. The Lavalink column is based on the shipped `Lavalink.jar`
4.2.2 artifact and the line-cited behavioural spec in
[`research/lavalink-source-architecture.md`](research/lavalink-source-architecture.md).
The NodeLink column is based on the analysed TypeScript tree recorded in
[`COMPATIBILITY.md`](COMPATIBILITY.md). The NEXORA column describes the code in
this repository at commit `b4346a2`, verified by `cargo test --workspace`
(370 tests, 0 failures).

The short version: **NEXORA is not a Lavalink wrapper, not a JVM application,
and not a Node.js application. It is a Rust audio server that offers a
Lavalink-shaped compatibility surface plus its own native contract.**

## At a glance

| Dimension | Lavalink 4.2.2 | NodeLink | NEXORA |
|---|---|---|---|
| Core language | Kotlin/Java on the JVM | TypeScript/JavaScript on Node.js | Rust |
| Packaging | Spring Boot fat JAR, Java 17, 133 nested jars | Node.js service tree (686 files, 185,577 TS LOC in the analysed tree) | Rust binary + library crates |
| Native runtime deps | Lavaplayer natives (~27 MB), `udpqueue-*`, platform `natives-*` | Node native addons per platform | No JVM, no Node runtime, no third-party audio service |
| Native API | `/v4/...` REST + `/v4/websocket` | NodeLink-specific routes plus a Lavalink-like surface | `/api/v1` native contract + Lavalink-compatible routes under `/nexora/*` |
| Control/audio split | Yes: bot sends voice state, node sends Opus | Yes | Yes; the client owns the voice gateway |
| Source plugins | Lavaplayer source managers + Java plugin API | Built-in TypeScript source managers, including commercial providers | Eight in-house Rust adapters, registered by the server |
| Extension model | Java/Kotlin plugin JARs compiled against `plugin-api` | TypeScript-level managers/routes | `wasmi` WASM modules under a manifest capability sandbox |
| Filters | 10 built-ins + Rhino JavaScript `pluginFilters` | 21 filters plus a mixer | 14 Rust filters; no JS filter engine, WASM is the substitute |
| Voice transport | Koe voice gateway + UDP + DAVE E2EE | NodeLink voice socket | Client-owned gateway; one UDP socket per player, RTP + AES-128-CTR |
| Session state | In-memory | In-memory | In-memory; opt-in snapshots via `--session-snapshot` |
| Metrics | JVM/stats over WebSocket | NodeLink-specific metrics | JSON `/api/v1/metrics` + Prometheus text `/metrics` |
| Cluster | External/multi-node setups | Two-tier worker model | Registry + weighted scheduler + explicit `mark_down` failover |
| Official SDK in this repo | Community client ecosystem | Node.js-native ecosystem | None shipped; `examples/bot_poc.py` is stdlib-only |
| Production readiness | Mature, widely deployed | Mature in its ecosystem | Not established; compatibility and benchmarks are published, production hardening remains |

## 1. Runtime and deployment

| | Lavalink | NodeLink | NEXORA |
|---|---|---|---|
| Runtime | JVM, Java 17 target | Node.js | Rust 1.100.0, edition 2024 |
| Entry point | `LavalinkApplication` / `Launcher`, Spring Boot | Node.js server process | `nexora-server` binary |
| Artifact shape | One fat JAR with nested dependency JARs | Source tree / npm-style deployment | One binary plus crates; Docker/compose assets in `deploy/` |
| Build | Gradle multi-project | npm/TypeScript toolchain | Cargo workspace, exact-pinned deps |
| Native codecs | Bundled platform-specific Lavaplayer natives | Platform-dependent Node modules | Rust crates compiled into the binary |
| CI proof | Upstream release pipeline | Upstream project pipeline | fmt + clippy + tests on Linux and Windows, docs site and link check |

**Why this matters:** Lavalink and NodeLink need their language runtime and
platform-native media dependencies on the host. NEXORA compiles those paths
into the Rust binary, so deployment is a single process with no JVM or Node
runtime.

## 2. API and protocol

| | Lavalink | NodeLink | NEXORA |
|---|---|---|---|
| REST prefix | `/v4` | NodeLink routes plus a Lavalink-like surface | `/api/v1` (native) and `/nexora/*` (compatibility) |
| Event WebSocket | `/v4/websocket`, server-push only | NodeLink event socket, plus voice/live sockets | `/nexora/websocket` (v4 shape) and `/ws/v1/sessions/{id}` (native envelope) |
| Auth | `Authorization` + `User-Id` headers | NodeLink client headers | Same Lavalink handshake for `/nexora/websocket`; Bearer auth on REST and both WebSockets |
| Op codes | `ready`, `stats`, `playerUpdate`, `event` | Similar plus NodeLink extras | Same four, with flat event envelopes |
| Event envelope | Flat: `op`, `type`, `guildId`, event fields as siblings | Flat | Flat, typed in `nexora-protocol` |
| Error envelope | Epoch-ms timestamp, status, error, trace, message, path | NodeLink-specific | Lavalink v4 shape |
| Encoded track | Lavaplayer binary v3 frame, base64 | NodeLink/opaque track blobs | Opaque NEXORA base64 JSON; **not** Lavalink binary |
| `decodetrack` | Yes, by clients with Lavaplayer semantics | Varies | Route exists; it decodes NEXORA's own blob, not Lavalink binary frames |
| Session resume | `Session-Id` header | Supported in NodeLink ecosystems | Implemented; player state and positions survive reconnect |

**Migration boundary:** a Lavalink client that only round-trips `encoded`
strings can use NEXORA's compatibility routes. A client that decodes Lavaplayer
binary tracks itself cannot. See
[`COMPATIBILITY.md`](COMPATIBILITY.md) §Protocol core.

## 3. Audio pipeline

| | Lavalink | NodeLink | NEXORA |
|---|---|---|---|
| Decode/source | Lavaplayer | NodeLink source managers | Rust decode/resample pipeline |
| PCM processing | `lavadsp` filters | 21 filter classes + mixer | 14 in-house Rust filters, applied in reference order |
| Opus | Lavaplayer/Opus encoders | Node encoders | Rust Opus path, 20 ms packets |
| Filters as scripts | Rhino JavaScript engine bundled in the JAR | Not the same model | No JS engine; WASM plugins are the extension boundary |
| Queue | Lavalink player queue | NodeLink queue | Bounded per-session queue with auto-advance |
| Live stream stalls | `TrackStuckEvent` meaningful | NodeLink stall handling | Pipeline is file/HTTP-buffered; `TrackStuck` shape is served but not a real live-stream stall |

NEXORA applies filters to PCM before Opus encoding, matching the reference
order: volume, equalizer, karaoke, timescale, tremolo, vibrato, distortion,
rotation, channelMix, lowPass, highpass, compressor, echo, phaser.

## 4. Sources

| | Lavalink | NodeLink | NEXORA |
|---|---|---|---|
| Local files | Yes | Yes | Yes, with path containment |
| HTTP/radio | Via Lavaplayer managers | Via managers | Yes, with SSRF-safe URL validation |
| YouTube/Spotify/Deezer/etc. | Source managers/plugins; YouTube often needs plugin config | Built-in managers; commercial providers often configured via credentials/API keys | In-house adapters for YouTube, Spotify, Deezer, Apple Music, Tidal, Bandcamp, SoundCloud, Internet Archive |
| Credential model | Plugin/config-specific | Provider config/API keys | User supplies their own authorized token; NEXORA ships no token and bypasses no access control |
| Source health | Not a first-class v4 route | NodeLink extras | `/nexora/sources/health` plus health events |
| Zero-config scope | Depends on plugins | Depends on provider setup | Tier A adapters need no config; credentialed providers need the user's account |

NEXORA deliberately does **not** reproduce NodeLink's remaining commercial
provider managers, HLS serving, profiler routes, or YouTube config/OAuth
surface. Those are recorded as `not-targeted` in the compatibility matrix.

## 5. Voice delivery

| | Lavalink | NodeLink | NEXORA |
|---|---|---|---|
| Voice gateway | Server-side, Koe | NodeLink voice socket | Client-owned; NEXORA has no voice gateway |
| UDP socket | Per player, managed by native `udpqueue` | NodeLink-managed | One `UdpSocket` per player, kept across track changes |
| Packet format | RTP + Opus | NodeLink's voice path | RTP v2, Opus payload type 120, +960 timestamp per 20 ms frame |
| Encryption | Koe path | NodeLink path | AES-128-CTR, key from the client-supplied voice token |
| DAVE E2EE | Bundled native library, inferred from `tink` | Varies | Not-targeted; protocol details are not observable from the shipped-artifact evidence |
| Address advertisement | Inside uninspected native code | NodeLink-specific | Not-targeted; the client owns that exchange |

This is the largest deliberate architectural difference. NEXORA sends audio to
the address implied by the client's voice state; it does not join Discord's
gateway or reproduce Koe's private handshake.

## 6. Plugins and extensions

| | Lavalink | NodeLink | NEXORA |
|---|---|---|---|
| Extension language | Java/Kotlin plugins against `plugin-api` | TypeScript managers and routes | WebAssembly modules |
| Loader | JVM plugin JAR bootstrap | Node module/source tree | Directory of `<name>.wasm` + `<name>.json` manifests |
| Sandbox | JVM classloader boundary | Node process boundary | Manifest capabilities, fuel time limit, memory limiter, host effect allow-list |
| Host effects | Plugin API hooks | Direct Node APIs | `host.fetch` and `host.push`; denied effects are returned, never silently granted |
| Can it add source adapters today? | Yes, via Java plugin ecosystem | Yes, in TypeScript | No: the ABI currently carries integers and strings, not source objects |

The WASM runtime is a substitute for the *extension shape*, not a claim that
Java plugins can run. See
[`COMPATIBILITY.md`](COMPATIBILITY.md) §Plugins and sandbox.

## 7. Events, metrics and operations

| | Lavalink | NodeLink | NEXORA |
|---|---|---|---|
| Core events | TrackStart/End/Exception/Stuck, WebSocketClosed, playerUpdate | 28 `GatewayEvents` including Seek/Pause/Volume/Filters/PlayerCreated | Core events plus NodeLink extras implemented where meaningful |
| Source health events | No | Yes | Yes: snapshot on first check, changes afterwards |
| Metrics | `stats` frame over WebSocket | NodeLink metrics | `stats` frame + JSON `/api/v1/metrics` + Prometheus `/metrics` |
| Session durability | In-memory | In-memory | In-memory with opt-in `--session-snapshot` startup/shutdown save/load |
| Cluster coordination | External | Two-tier workers | Registry, weighted scheduler, `mark_down` failover |
| Benchmarks | Upstream numbers | Upstream numbers | Published Criterion methodology in [`BENCHMARKS.md`](BENCHMARKS.md) |

## 8. Client ecosystem

| | Lavalink | NodeLink | NEXORA |
|---|---|---|---|
| Official SDK in this repo | Not applicable | Not applicable | None; `sdks/` was removed |
| Integration path | Mature third-party Lavalink clients | Node.js-native clients | `/api/v1` contract, `/nexora/*` compatibility surface, OpenAPI/protobuf contracts in `crates/nexora-protocol` and `proto/nexora.proto` |
| Example | Bot connects to node over REST/WS | Node bot uses NodeLink library | [`examples/bot_poc.py`](../examples/bot_poc.py) uses only the Python standard library |

## When to choose what

- **Choose Lavalink** if you need the mature JVM ecosystem, Java plugins,
  Lavaplayer's binary track format, or server-side Koe/DAVE voice handling.
- **Choose NodeLink** if you want a Node.js-native service with its built-in
  commercial provider managers, HLS, profiler, worker model, or mixer.
- **Choose NEXORA** if you want one Rust binary, no JVM/Node runtime, an
  in-house source layer, a documented native API, and a sandboxed WASM
  extension model — and you accept that it is not yet production-hardened and
  does not reproduce every Lavalink/NodeLink feature.

## Migration checklist from Lavalink to NEXORA

1. Point the client at `/nexora/*` instead of `/v4/*`; keep the Bearer password
   and `User-Id` handshake.
2. Treat `encoded` as opaque; round-trip it through NEXORA. Do not decode it
   with a Lavaplayer library.
3. Expect voice delivery without a NEXORA voice gateway: the client supplies
   voice state and owns the gateway connection.
4. Replace Java plugin JARs with WASM modules, or use the native `/api/v1`
   routes directly.
5. Replace JavaScript filters with NEXORA's Rust filter chain.
6. Re-run the compatibility suite in `services/nexora-server/tests/compat/`;
   do not assume parity from route names alone.

## Status honesty

The compatibility matrix counts 63 implemented rows, 5 partial rows, 0
missing rows and 21 deliberate `not-targeted` rows. Those counts are matrix
rows, not a promise of full client conformance. A row is implemented only when
a test exercises it against the reference behavior. Production readiness,
full third-party client conformance, and credentialed live-provider runs are
still open.
