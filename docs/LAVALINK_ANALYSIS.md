# Lavalink Analysis (reference for NEXORA)

Status: v4.2.2 documentation plus static REA inspection of the shipped
`Lavalink.jar` (SHA-256 `8cb801e591072c36...`, REA evidence
`ev_80dae1e96851ebd5550a80e9ba753eef7b603d1a2c0a81b0395cdfced5a1d3d6`).
Evidence boundary: the JAR inventory was observed statically; it does not prove
runtime behavior. Protocol behavior below comes from the official v4 docs.

## 1. What Lavalink is

A standalone audio-sending node, written mostly in Kotlin on the JVM. A Discord
bot connects to it over REST and WebSocket; audio never reaches the bot's
gateway process. This separation is the idea worth keeping: control plane and
audio plane are separate, and the engine can serve any client, not only Discord.

In v4 the WebSocket is server-push only: all client commands go through REST.
The REST prefix is `/v4`; unprefixed routes were removed.

## 2. Observed request lifecycle (official docs)

1. Client opens `/v4/websocket` with `Authorization`, `User-Id`,
   `Client-Name`, optionally `Session-Id` for resume. Server sends `ready`
   with a session id.
2. Client calls `GET /v4/loadtracks?identifier=...`. Source managers resolve
   the identifier into `track`, `playlist`, `search`, `empty`, or `error`.
   Tracks carry an opaque base64 `encoded` blob plus metadata.
3. Client creates a session and sends `PATCH /v4/sessions/{id}/player` with
   the encoded track and voice state (token, endpoint, session id, channel id).
4. Inside the node: Lavaplayer resolves and decodes the source, produces PCM
   frames, applies filters (volume, equalizer, karaoke, timescale, tremolo,
   vibrato, distortion, rotation, channel mix, low pass), then encodes 20 ms
   Opus packets.
5. Koe sends those packets to Discord's voice server over its voice
   WebSocket/UDP path. v4.2.0 added DAVE (E2EE voice).
6. Server pushes `playerUpdate` (position, connected, ping), `stats`
   (memory, CPU, frame sent/nulled/deficit per player), and `event`
   (track start/end/exception/stuck, WebSocket closed) over the WebSocket.
7. `stats.frameStats` expects 3000 frames per minute per player (one per
   20 ms); a positive deficit means frames were not delivered on time.

## 3. Shipped-artifact structure (REA observation)

The artifact is a Spring Boot fat JAR: 986 ZIP entries, 329 nodes in the
inventory graph. Observed under `BOOT-INF/classes/lavalink/server/`:

- `Launcher.class`, `LavalinkApplication.class`
- `bootstrap/`: `PluginManager`, `PluginDeclaration`, `Versioning`
- `config/`: `AudioPlayerConfiguration`, `KoeConfiguration`, `ServerConfig`,
  `RateLimitConfig`, `RequestAuthorizationFilter`, `YoutubeConfig`
- `info/InfoRestHandler.class`
- `io/`: `EventEmitter`, `SessionRestHandler`, `RoutePlannerRestHandler`,
  `HandshakeInterceptorImpl`, request-logging and response-header filters
- `application.yml` and `application.yml.example`

Inference (not an observation): this layout reflects config/auth -> REST/WS
handlers -> player -> voice sender. Treat that arrow as a design hypothesis.

## 4. Invariants NEXORA should preserve

- 20 ms Opus framing with monotonic sequence and timestamps.
- Bounded buffers, explicit backpressure, and frame-deficit style monitoring.
- Typed errors and one consistent JSON error envelope (`code` + `message`).
- Token/bearer authentication at the trust boundary; local media stays inside
  a configured root; outbound URLs are validated against SSRF.
- Session-scoped event broadcast and resumable sessions.
- Source abstraction with capability flags, timeouts, and cancellation.
- Compatibility surfaces are adapters, never the core contract.

NEXORA already implements the last three in `nexora-source-api`,
`nexora-source-framework`, and `nexora-session`.

## 5. Deliberate non-goals

- JVM/plugin classloading and Maven bootstrap.
- Route-planner IP rotation, rate-limit evasion, or search-trigger circumvention.
- Proprietary YouTube/Spotify extraction without authorized APIs.
- Copying Lavalink's internal types as our public contract. `/v4` exists in
  NEXORA as a compatibility layer; `/api/v1` remains the long-term contract.

## 6. Dependency policy — material tradeoff

"Independent" means no runtime dependence on a third-party *service* or
platform: no hosted API, no scraper, no external node, no credentials to run.
Rust crates such as tokio, axum, symphonia, and opus-rs are compile-time
libraries. Replacing them with hand-written Opus, TLS, HTTP and container
code would be a separate multi-year project and would not improve
independence from services. This tradeoff is recorded explicitly; a stricter
zero-crate rewrite would need a fresh scope decision.

## 7. Workflow

Use the rust-anything discipline: this document records the original's
observable behavior; change one unit at a time and prove parity with contract
tests before touching protocol-facing code. Re-run REA inspection when a new
Lavalink release changes the shipped artifact.

First parity unit (done): `/v4/loadtracks` now returns v4 lowercase
`loadType` values (`empty` for no results, `search` for results) instead of the
old non-standard `SEARCH_RESULT`, and `/v4/stats` uses `cpu.lavalinkLoad`
instead of `cpu.processLoad`. Two RED-GREEN contract tests were added.

Second parity unit (done): `/v4/loadtracks` now maps every result to the v4
track shape: `encoded`, `info` (identifier, isSeekable, author, length,
isStream, position, title, uri, artworkUrl, isrc, sourceName), `pluginInfo`,
`userData`. The `encoded` string is an opaque NEXORA base64 JSON blob, not
Lavalink's binary track format; clients must round-trip it through NEXORA
rather than decode it with a Lavalink library. Integration test uses a real
local-media adapter and decodes the blob to prove it is non-empty.

Remaining gap: Lavalink binary track encoding and `/v4/decodetrack` are not
implemented. That is a material compatibility boundary, not an accident.

Next unit: contract tests comparing `/v4/info`, `/v4/loadtracks`, and
player response shapes against the documented v4 shapes, followed by the
Discord bot proof of concept.
