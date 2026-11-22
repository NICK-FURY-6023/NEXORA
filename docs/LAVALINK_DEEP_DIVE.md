# NEXORA — Lavalink v4.2.2 Deep Dive

**Purpose:** Document exactly what Lavalink is, what is inside it, and how it works, so NEXORA can build an independent lightweight Rust equivalent — not a wrapper, not a JVM port, zero third-party runtime dependency.

**Evidence standard:** Every claim carries an **OBSERVED / INFERRED** label. OBSERVED = directly verified from the downloaded/verified Lavalink v4.2.2 JAR (SHA-256 `8cb801e591072c3689fafd71ccf571a95a4ead3cc35dfc045e157d763d89119a`), its extracted classes, `javap` signatures, `REA` archive extraction, and/or the cloned v4.2.2 Kotlin source. INFERRED = reasoned, not proven by static analysis.

**Mandatory skills used:** reverse-engineer-anything (REA), rust-anything, ecc-guide, using-superpowers (global memory `mem_1795359430838_11883272316274892010`).

---

## 1. What Lavalink is (OBSERVED)

Lavalink v4.2.2 is a standalone **audio forwarding service for Discord music bots**. It is **not** a music source itself — it does not own YouTube/SoundCloud. It is a server that:

1. Accepts **REST** commands (`/v4/...`) from a bot client (password-authenticated).
2. Accepts a **WebSocket** (`/v4/websocket`) from the same client for push events (track start/end/stuck/exception, player state updates, node stats).
3. Resolves arbitrary identifiers (URLs, `ytsearch:`, `scsearch:`, etc.) into playable audio tracks via pluggable **source managers** (backed by Lavaplayer).
4. Decodes those tracks to PCM, applies an ordered **filter chain** (volume, EQ, karaoke, timescale, tremolo, vibrato, distortion, rotation, channel mix, low-pass + plugin filters), resamples to Discord's Opus format.
5. Connects to a Discord voice gateway (UDP, Opus, NaCl/secret-box encryption) and streams 20 ms frames.

**Key architectural fact (OBSERVED):** Lavalink is **stateless about music catalog** — it proxies/decodes on demand. All "intelligence" (what a track is, how to decode it) lives in Lavaplayer + source managers; Lavalink itself is a session/player/websocket/REST orchestrator around Lavaplayer.

**Source of truth for this section:** `docs/research/lavalink-rea-static-analysis.md` (REA), official `lavalink.dev/api/rest.html`, `lavalink.dev/api/websocket.html`, cloned `lavalink-devs/Lavalink@4.2.2` source.

---

## 2. What is inside Lavalink (OBSERVED)

### 2.1 Artifact identity
| Field | Value |
|-------|-------|
| SHA-256 | `8cb801e591072c3689fafd71ccf571a95a4ead3cc35dfc045e157d763d89119a` |
| Format | Spring Boot fat JAR = ZIP (~100 MB, 328 entries) |
| Version | 4.2.2 (`BOOT-INF/classes/app.properties`) |
| Build timestamp | `1772839456827` ms epoch |
| JVM target | JVM 17 (Kotlin 2.1.20) |

### 2.2 Runtime dependencies (132 jars in `BOOT-INF/lib/`)
The dependency set is what a Rust reimplementation must *replace*, not reuse. Grouped:

| Area | Jars (sample) | What they provide |
|------|---------------|-------------------|
| App framework | spring-boot 3.3.0, spring-webmvc 6.1.8, spring-websocket 6.1.9, tomcat/undertow | HTTP + WS server |
| JSON | jackson 2.17.1, kotlinx-serialization 1.7.3, gson | (de)serialization |
| Audio engine | lavaplayer 2.2.6, lavadsp 0.7.8, lava-common 2.2.6, lavaplayer-natives 2.2.6 | track loading, decode, playback, DSP filters |
| Voice gateway | koe (transitive), netty 4.2.10 | Discord voice UDP + Opus + encryption |
| HTTP client | httpclient 4.5.14, httpclient5 5.2.3 | source fetching |
| Crypto | bcprov-jdk18on 1.78, tink 1.14.1 | voice encryption, signing |
| Logging/metrics | logback/log4j/slf4j, micrometer, sentry | observability |
| System stats | oshi-core 6.4.11 | cpu/memory for `/v4/stats` |
| Config | snakeyaml 2.2 | YAML |
| Plugin | plugin-api-4.2.2.jar | plugin extension points |

**Rust implication (INFERRED):** Replace spring-boot/webmvc/websocket with axum + tokio-tungstenite; replace lavaplayer/lavadsp with a custom audio pipeline; replace koe/netty voice with a custom UDP voice-gateway client. Keep only std + serde + audio-codec crates. No JVM, no Spring, no Lavaplayer at runtime.

### 2.3 Class inventory (lavalink/server — 98 classes)

| Package | Classes | Responsibility |
|---------|---------|----------------|
| `lavalink/server` | 2 | `LavalinkApplication`, `Launcher` (entry point) |
| `bootstrap` | 17 | `PluginManager`, `PluginManifest`, `PluginsConfig` — jar download + `URLClassLoader` plugin loading |
| `config` | 18 | `ServerConfig` (`lavalink.server.*`), audio player/source configs |
| `info` | 5 | `AppInfo`, `GitRepoState` — `/v4/info` |
| `io` | 21 | `SocketServer` (WS `/v4/websocket`), `SocketContext` (per-session state) |
| `metrics` | 5 | `StatsCollector` — `/v4/stats` |
| `player` | 12 (+15 filters) | `LavalinkPlayer`, `EventEmitter`, `AudioLoaderRestHandler`, `PlayerRestHandler`, `FilterChain` |
| `util` | 3 | `encodeTrack`/`decodeTrack` (Base64 over Lavaplayer `MessageOutput`/`MessageInput`), `loadAudioItem` |

**Source-manager classes are NOT in this module** — they live in `lavaplayer-2.2.6.jar` (`com.sedmelluq.discord.lavaplayer.source.*`). Lavalink server only wires an `AudioPlayerManager`.

---

## 3. How Lavalink works (OBSERVED + INFERRED)

### 3.1 Entry point (OBSERVED, `Launcher.kt`)
`Launcher.main` runs **two** Spring Boot contexts:
1. **Plugin bootstrap** (`PluginManager`, `WebApplicationType.NONE`): downloads plugin jars from maven (`{repository}{group}/{name}/{version}/{name}-{version}.jar`), verifies `lavalink-plugins/*.properties` manifests (`name`, `path`, `version`), loads jars into a child `URLClassLoader`, builds `pluginManifests`.
2. **Main web app** (`LavalinkApplication`, child of plugin context): `componentScan` = plugin manifest paths + `lavalink.server`; `resourceLoader` uses plugin classloader so plugin `@Service` beans resolve. Beans: `SocketServer`, `AudioLoaderRestHandler`, `PlayerRestHandler`, `StatsCollector`, `FilterChain`, per-guild `LavalinkPlayer`, per-player `EventEmitter`.

### 3.2 Session & player model (OBSERVED)
- A **session** = 16-char alnum `sessionId`, created on WS connect (headers: `Authorization`, `User-Id`, `Client-Name`, optional `Session-Id` for resume).
- `SocketServer.sessions: ConcurrentHashMap<sessionId, SocketContext>`; `resumableSessions` for resume within timeout (default 60s).
- A **player** = per `guildId` inside a session: `AudioPlayer` (Lavaplayer), `FilterChain`, `MutableAudioFrame` provider, scheduled `playerUpdate` (interval `playerUpdateInterval`, default 5s).
- `SocketContext` holds: players map, media connection, voice server info (token/endpoint/sessionId/channelId), event emitter, player-update scheduler.

### 3.3 REST surface (OBSERVED, docs + `AudioLoaderRestHandler`/`PlayerRestHandler`)
All `/v4/...`, password via `Authorization` header:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v4/loadtracks?identifier=` | GET | resolve track / playlist / search → `LoadResult` (loadType: track|playlist|search|empty|error) |
| `/v4/decodetrack?encodedTrack=` | GET | decode one track → `Track` |
| `/v4/decodetracks` | POST | decode array of tracks |
| `/v4/sessions/{sessionId}/players` | GET | list players |
| `/v4/sessions/{sessionId}/players/{guildId}` | GET | get player |
| `/v4/sessions/{sessionId}/players/{guildId}?noReplace=` | PATCH | update/create player (track, volume, paused, filters, voice, position, endTime) |
| `/v4/sessions/{sessionId}/players/{guildId}` | DELETE | destroy player |
| `/v4/sessions/{sessionId}` | PATCH | update session (resuming, timeout) |
| `/v4/info` | GET | version/build/git/jvm/lavaplayer/sourceManagers/filters/plugins |
| `/version` | GET | plain version string |
| `/v4/stats` | GET | players, playingPlayers, uptime, memory, cpu |
| `/v4/routeplanner/{status,free/address,free/all}` | GET/POST | IP rotation (bans) |

### 3.4 WebSocket surface (OBSERVED, docs + `SocketServer`)
Path `/v4/websocket`; client→server text messages **not supported** (logs warning, uses REST only). Server→client ops:

| op | Meaning |
|----|---------|
| `ready` | session id + resumed flag |
| `playerUpdate` | periodic player state (time, position, connected, ping) |
| `stats` | per-minute node stats (memory, cpu, frameStats) |
| `event` | TrackStart / TrackEnd / TrackException / TrackStuck / WebSocketClosed |

### 3.5 Audio pipeline (OBSERVED structure, INFERRED internals)
Track load: `identifier` → `AudioPlayerManager.loadItemSync` → `AudioTrack`/`AudioPlaylist` → encoded via `encodeTrack` (Lavaplayer `MessageOutput` → Base64) → sent to client as `encoded` blob.

Playback: client sends `PATCH /v4/players/{guildId}` with `encodedTrack` → `decodeTrack` (Base64 → `MessageInput` → `AudioTrack`) → `audioPlayer.playTrack(track)` → Lavaplayer decodes to PCM → `FilterChain.buildChain()` applies filters in fixed order:

1. volume
2. equalizer (15 bands: 25Hz…16kHz, gain −0.25…1.0)
3. karaoke
4. timescale (speed/pitch/rate)
5. tremolo
6. vibrato
7. distortion
8. rotation
9. channelMix
10. lowPass
11. pluginFilters (last)

→ resample to Discord Opus (48 kHz stereo, 20 ms frames) → `AudioFrameProvider` writes Opus frames to `ByteBuf` → sent over Discord voice UDP gateway.

**DAVE E2EE voice (v4.2.0+):** Voice state carries an additional `channelId` field for Discord's DAVE (Discord Audio Voice Encryption) end-to-end encryption. (OBSERVED from changelog + protocol `VoiceState.channelId?: String?`.)

Events: `EventEmitter` (`AudioEventAdapter`) → `onTrackStart`/`onTrackEnd`/`onTrackException`/`onTrackStuck` → WS `event` op + `playerUpdate`.

### 3.6 Encoded-track format (OBSERVED mechanism, INFERRED byte layout)
`encodeTrack(audioPlayerManager, track)`:
1. `MessageOutput(baos)` → `audioPlayerManager.encodeTrack(output, track)` → writes Lavaplayer's internal serialized `AudioTrack` to `ByteArrayOutputStream`.
2. `Base64.getEncoder().encodeToString(baos.toByteArray())` → the `encoded` string seen in every API payload.

`decodeTrack`: `Base64.getDecoder().decode(message)` → `ByteArrayInputStream` → `MessageInput(bais)` → `audioPlayerManager.decodeTrack(input).decodedTrack`.

**INFERRED:** The `MessageOutput`/`MessageInput` binary layout is Lavaplayer-private (track identifier + `AudioTrackInfo` + source-specific decoded data + position). It was **not** decompiled here (REA archive providers reject ZIP; only `extract-artifact` + `javap` used). For protocol compatibility NEXORA must define its own binary track container and document it — it need not be byte-compatible with Lavaplayer, only semantically equivalent (identifier + info + opaque source payload).

---

## 4. Protocol surface NEXORA should mirror (for client compatibility)

| Concept | Lavalink v4 | NEXORA equivalent (Rust) |
|---------|-------------|--------------------------|
| Auth | `Authorization: <password>` header | same |
| Session id | 16-char alnum, WS header `Session-Id` for resume | same |
| REST prefix | `/v4/...` | NEXORA native `/api/v1` + optional `/v4` compat (ADR-016) |
| WS path | `/v4/websocket` | `/api/v1/websocket` |
| WS ops | ready, playerUpdate, stats, event | same set |
| Event types | TrackStart/End/Exception/Stuck/WebSocketClosed | same |
| Track object | `{encoded, info{...}, pluginInfo{}, userData{}}` | same shape, Rust `encoded` format |
| Filters | volume, equalizer(15 bands), karaoke, timescale, tremolo, vibrato, distortion, rotation, channelMix, lowPass, pluginFilters | same, same order |
| Player state | `{time, position, connected, ping}` | same |
| Stats | players, playingPlayers, uptime, memory, cpu, frameStats | same |
| Volume range | 0–1000 % | same |
| playerUpdate interval | default 5s | same |

---

## 5. Gaps / unknowns (honest reporting)

1. **Lavaplayer binary track format** not reverse-engineered — `MessageOutput`/`MessageInput` byte layout unknown. NEXORA defines its own container.
2. **koe / Discord voice gateway internals** not inspected (class present, internals not read). UDP + Opus + NaCl encryption must be implemented custom.
3. **Runtime thread-pool sizes, frame pacing, resume-timeout defaults** not all read from source. Defaults to be chosen by NEXORA.
4. **Source managers (YouTube, SoundCloud, HTTP, local)** live in Lavaplayer, not in the server jar. NEXORA must implement its own source resolution (or none — out of scope for a lightweight core).
5. **Static analysis cannot prove runtime behaviour** (dependency loading, network timing, GC). All runtime claims are INFERRED.

---

## 6. What NEXORA keeps vs drops (from ADR-015/016/017/020)

| Keep | Drop / Replace |
|------|----------------|
| Session → players → guild scoping | JVM / Spring Boot |
| REST + WS split (REST for commands, WS for events) | Lavaplayer (custom Rust audio pipeline) |
| Ordered filter chain (10 built-in + plugin) | koe / netty voice (custom UDP voice client) |
| Opaque `encoded` track blob (own format) | Maven/plugin classpath loading (Rust `libloading`/WASM traits) |
| Password auth | `application.yml` complexity (NEXORA uses own config) |
| In-memory sessions (ADR-017), bounded queues (ADR-020) | — |

**NEXORA direction:** From-scratch Rust audio engine + service platform. Zero third-party *runtime* dependency (no JVM, no Spring, no Lavaplayer service). Native Rust codec crates acceptable as libraries.
