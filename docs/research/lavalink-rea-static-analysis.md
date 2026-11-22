# Lavalink v4.2.2 — REA Static Analysis (JAR)

**Target artifact:** `/home/nickfury6023/.jcode/scratch/lavalink-re/Lavalink.jar`
**SHA-256:** `8cb801e591072c3689fafd71ccf571a95a4ead3cc35dfc045e157d763d89119a` — **verified** against `docs/LAVALINK_ANALYSIS.md` evidence `ev_80dae1e96851ebd5550a80e9ba753eef7b603d1a2c0a81b0395cdfced5a1d3d6`.
**Format:** Spring Boot fat JAR = ZIP archive (~100 MB, 328 zip entries / 329 graph nodes).
**Build time:** `1772839456827` ms epoch → version `4.2.2` (`BOOT-INF/classes/app.properties`).
**JVM target:** JVM 17 (`kotlin.jvmTarget = JVM_17` per `build.gradle.kts`); Kotlin 2.1.20.
**REA extraction:** `rea extract-artifact Lavalink.jar --integrity-policy record-and-continue --format json` → `rea-extract.json` (304 artifacts, containment verified, no integrity contradictions).
**Materialized output root:** `/home/nickfury6023/.jcode/scratch/rea-extracted-9740d6c8-9749-4c77-9b71-f810dab830fe` (`$R`).
**Source mirror read:** `/home/nickfury6023/.jcode/scratch/lavalink-src/` (cloned `lavalink-devs/Lavalink@4.2.2`).
**Lavaplayer source mirror:** `/home/nickfury6023/.jcode/scratch/lavaplayer-src/` (tag 2.2.6).

> Constraint labels used below:
> - **OBSERVED:** directly seen in extracted bytes/classes/properties/source.
> - **DERIVED:** computed from observed data (package counts, version aggregates).
> - **INFERRED:** reasoned about likely behaviour; static analysis does not prove runtime behaviour.

---

## 1. Artifact identity

| Field | Value | Label |
|-------|-------|-------|
| SHA-256 | `8cb801e591072c3689fafd71ccf571a95a4ead3cc35dfc045e157d763d89119a` | OBSERVED |
| Format | ZIP / Spring Boot fat JAR | OBSERVED |
| Zip entries | 328 | OBSERVED |
| Graph nodes (REA) | 329 | OBSERVED |
| Version | 4.2.2 (`app.properties`) | OBSERVED |
| Build timestamp | `1772839456827` ms epoch | OBSERVED |
| Group / artifact | `dev.arbjerg.lavalink` / `Lavalink-Server` | OBSERVED |
| JVM target | JVM 17 | DERIVED (from build config) |
| Kotlin | 2.1.20 | DERIVED (from build config) |
| REA extraction artifacts | 304 materialized files | OBSERVED |
| REA containment verified | true | OBSERVED |
| Integrity contradictions | none | OBSERVED |

---

## 2. Package / class inventory (lavalink/server — 98 classes)

Derived from `$R/BOOT-INF/classes/lavalink/` (98 class files) — recursive directory listing, matched to source mirror.

| Package | Class count | Sub-packages |
|---------|-------------|--------------|
| `lavalink.server` | 2 | — |
| `lavalink.server.bootstrap` | 17 | — |
| `lavalink.server.config` | 18 | — |
| `lavalink.server.info` | 5 | — |
| `lavalink.server.io` | 21 | — |
| `lavalink.server.metrics` | 5 | — |
| `lavalink.server.player` | 12 | `filters` (15) |
| `lavalink.server.util` | 3 | — |

### Full class list (OBSERVED from extracted classes + source)

**`lavalink.server` (2):** `Launcher`, `LavalinkApplication`

**`lavalink.server.bootstrap` (17):** `MavenKt`, `Metadata` (+ serializer companions), `PluginDeclaration`, `PluginManager` (+ `Companion`, `Declaration`, `PluginJar`), `PluginManifest`, `PluginsConfig`, `Versioning` (+ serializer companions)

**`lavalink.server.config` (18):** `AudioPlayerConfiguration`, `AudioSourcesConfig`, `HttpConfig`, `KoeConfiguration`, `MetricsPrometheusConfigProperties`, `RateLimitConfig`, `RequestAuthorizationFilter` (+ `Companion`), `RequestLoggingConfig`, `SentryConfigProperties`, `SentryConfiguration` (+ `Companion`), `ServerConfig`, `TimeoutsConfig`, `WebConfiguration`, `WebsocketConfig` (+ `Companion`), `YoutubeConfig`

**`lavalink.server.info` (5):** `AppInfo` (+ `Companion`), `GitRepoState` (+ `Companion`), `InfoRestHandler`

**`lavalink.server.io` (21):** `DefaultAudioPluginInfoModifier` (+ `Companion`), `EventEmitter` (+ `Companion`), `HandshakeInterceptorImpl` (+ `Companion`), `RequestLoggingFilter` (+ `Companion`), `ResponseHeaderFilter`, `RoutePlannerRestHandler` (+ `RoutePlannerDisabledException`), `SessionRestHandler`, `ShutdownHandler`, `SocketContext` (+ `Companion`, `send$1`, `WsEventHandler`), `SocketServer` (+ `Companion`), `StatsCollector` (+ `Companion`)

**`lavalink.server.metrics` (5):** `GcNotificationListener`, `LavalinkStatsCollector` (+ `Companion`), `PrometheusMetrics`, `PrometheusMetricsController`

**`lavalink.server.player` (12):** `AudioLoaderRestHandler` (+ `Companion`), `AudioLossCounter` (+ `Companion`), `EventEmitter` (+ `Companion`), `LavalinkPlayer` (+ `Provider`), `PlayerRestHandler` (+ `Companion`), `TrackEndMarkerHandler` (+ `Companion`), plus `filters` package (15):

- `lavalink.server.player.filters` (15): `Band`, `ChannelMixConfig`, `DistortionConfig`, `EqualizerConfig`, `FilterChain` (+ `Companion`, `PluginConfig`), `FilterConfig`, `KaraokeConfig`, `LowPassConfig`, `RotationConfig`, `TimescaleConfig`, `TremoloConfig`, `VibratoConfig`, `VolumeConfig`

**`lavalink.server.util` (3):** `LoadingKt`, `UtilKt` (+ `WhenMappings`)

### Not present / not shipped
`lavalink/server/audio/` source-manager implementation classes are **NOT** in this server module — they live in Lavaplayer (`lavaplayer-2.2.6.jar`) under `com.sedmelluq.discord.lavaplayer.source.*`. The server only wires an `AudioPlayerManager`.

---

## 3. Runtime dependency inventory (132 jars in `BOOT-INF/lib/`)

Grouped by what they provide and what a Rust replacement would need.

### Core framework (would NOT be reused in Rust; Rust is from-scratch)

| Jar | Version | Provides | Rust replacement candidate | Label |
|-----|---------|----------|---------------------------|-------|
| `spring-boot` | 3.3.0 | app framework | — (Rust std + custom) | OBSERVED |
| `spring-boot-autoconfigure` | 3.3.0 | autoconfig | — | OBSERVED |
| `spring-webmvc` | 6.1.8 | REST controllers | axum / actix-web | INFERRED |
| `spring-websocket` | 6.1.9 | WS server | tokio-tungstenite | INFERRED |
| `tomcat-embed-el` | 10.1.24 | embedded servlet | — (custom HTTP) | INFERRED |
| `undertow-core` | 2.3.13 | servlet/WS container | — | OBSERVED |
| `jackson-*` | 2.17.1 | JSON (de)serialization | serde_json | INFERRED |
| `kotlin-stdlib` / `kotlin-reflect` | 2.1.20 | Kotlin runtime | — (Rust std) | OBSERVED |
| `kotlinx-serialization-*` | 1.7.3 | Kotlin serialization | serde | INFERRED |
| `logback-*` / `log4j-*` / `slf4j-api` | 1.5.6 / 2.23.1 / 2.0.17 | logging | tracing / log | INFERRED |
| `micrometer-*` | 1.12.12 | metrics | custom metrics | INFERRED |
| `sentry-*` | 7.10.0 | error reporting | — (optional) | INFERRED |

### Audio / DSP (the heart of what Rust must reimplement)

| Jar | Version | Provides | Rust replacement candidate | Label |
|-----|---------|----------|---------------------------|-------|
| `lavaplayer` | 2.2.6 | track loading, decoding, playback engine | **custom audio pipeline** | OBSERVED |
| `lavadsp` | 0.7.8 | audio DSP filters | **custom DSP** | OBSERVED |
| `lava-common` | 2.2.6 | lavaplayer common | **custom** | INFERRED |
| `lavaplayer-natives` | 2.2.6 | native codecs (opus, mp3, flac, vorbis) | **native codec crates** | OBSERVED |
| `lavaplayer-ext-youtube-rotator` | 2.2.6 | YouTube IP rotation | **custom HTTP client + IP rotation** | INFERRED |
| `native-loader` / `natives-*` | 0.0.1 / 0.1.3 | platform native libs | Rust native deps / static linking | INFERRED |
| `bcprov-jdk18on` | 1.78 | Bouncy Castle crypto | **rust-crypto / ring** | INFERRED |
| `tink` | 1.14.1 | Google Tink crypto | — | INFERRED |
| `jsoup` | 1.16.1 | HTML parsing | **scraper / select** | INFERRED |
| `gson` | 2.10.1 | JSON | serde_json | INFERRED |
| `protobuf-java` | 3.25.3 | protobuf | prost | INFERRED |
| `oshi-core` | 6.4.11 | system stats (cpu/mem) | **custom sysinfo** | INFERRED |
| `koe` (media connection) | (transitive) | Discord voice gateway (UDP, opus, encryption) | **custom UDP + libsodium + opus** | INFERRED |

### HTTP / networking / IP rotation

| Jar | Version | Provides | Rust replacement candidate | Label |
|-----|---------|----------|---------------------------|-------|
| `httpclient` / `httpclient5` | 4.5.14 / 5.2.3 | Apache HTTP client | **reqwest / ureq** | INFERRED |
| `httpcore` / `httpcore5` | 4.4.16 / 5.2.4 | HTTP core | — | OBSERVED |
| `netty-*` | 4.2.10 | NIO networking (discord voice) | tokio | INFERRED |
| `udpqueue-*` | 0.2.12 | UDP queue / rate limiting | **custom** | INFERRED |

### Other

| Jar | Version | Provides | Label |
|-----|---------|----------|-------|
| `jakarta.servlet-api` | 6.0.0 | servlet API | OBSERVED |
| `jakarta.websocket-api` | 2.1.0 | WS API | OBSERVED |
| `commons-io` / `commons-codec` | 2.13.0 / 1.11 | IO / codec utils | OBSERVED |
| `jna` / `jna-platform` | 5.14.0 | JNI / native access | OBSERVED |
| `snakeyaml` | 2.2 | YAML config | OBSERVED |
| `spring-security-crypto` / `spring-security-rsa` | 6.2.8 / 1.1.3 | password hashing / RSA | OBSERVED |
| `plugin-api-4.2.2.jar` | 4.2.2 | plugin extension points | OBSERVED |

**Key takeaway for Rust:** Lavalink's runtime value is ~80% Lavaplayer (audio pipeline) + Spring (web/WS framing) + koe (Discord voice). A from-scratch Rust system replaces Spring with axum/tokio-tungstenite, Lavaplayer with a custom audio pipeline, koe with a custom voice-gateway client, and keeps only std + serde + a crypto/audio codec crate set. Zero third-party *runtime* dependency in the user's sense means: no JVM, no Spring, no Lavaplayer — but audio codecs (opus, etc.) are acceptable as native Rust crates since they are libraries, not an external runtime service.

---

## 4. Entry point wiring (OBSERVED from `Launcher.kt`)

`Launcher.main` runs **two** Spring Boot contexts:

1. **Plugin bootstrap** (`PluginManager`, `WebApplicationType.NONE`): runs `PluginManager` as a separate Spring context with banner off. This downloads/verifies/loads plugin jars into a child `URLClassLoader`, builds `pluginManifests` (classpath `lavalink-plugins/*.properties` + plugin dir jars).
2. **Main web app** (`LavalinkApplication`, child context of the plugin context):
   - `componentScan` = plugin manifest paths + `lavalink.server`
   - `resourceLoader` uses `pluginManager.classLoader` (so plugin classes are resolvable)
   - Listeners log version info on env-prepared, "ready" on ready, error on failed.

**Bean wiring:** `SocketServer` (WebSocket handler), `AudioLoaderRestHandler` (+ `PlayerRestHandler`), `StatsCollector`, `FilterChain`, `EventEmitter` (per player), `LavalinkPlayer` (per guild) are all Spring-managed singletons/prototypes wired via constructor injection.

### Call graph — entry points (derived from source reads)

```
Launcher.main
  ├─ launchPluginBootstrap → PluginManager
  │     ├─ manageDownloads() — download plugin jars from Maven
  │     ├─ readClasspathManifests() — scan classpath*:lavalink-plugins/*.properties
  │     └─ loadJars() — build URLClassLoader, load plugin classes
  └─ launchMain(parent) → LavalinkApplication (componentScan)
        ├─ SocketServer        — WS /v4/websocket, session lifecycle
        ├─ AudioLoaderRestHandler — GET /v4/loadtracks, /v4/decodetrack, POST /v4/decodetracks
        ├─ PlayerRestHandler   — PATCH/DELETE /v4/sessions/{sid}/players/{gid}
        ├─ SessionRestHandler  — PATCH /v4/sessions/{sid}
        ├─ RoutePlannerRestHandler — GET/POST /v4/routeplanner/{...}
        ├─ InfoRestHandler     — GET /v4/info, GET /version
        ├─ StatsCollector      — GET /v4/stats + periodic stats task
        └─ FilterChain         — filter parsing/build
```

### Handler → internal call chains

```
WS connect (SocketServer.afterConnectionEstablished)
  ├─ generateUniqueSessionId() (16-char alnum [a-z0-9])
  ├─ resume existing SocketContext from resumableSessions
  │     └─ SocketContext.resume(session) → replay resumeEventQueue + sendPlayerUpdate per player
  └─ new SocketContext(sessionId, audioPlayerManager, serverConfig, session, socketServer,
                        statsCollector, userId, clientName, koe.newClient(userId), ...)
        ├─ SocketContext init → statsCollector.createTask scheduled every 1 min
        │     └─ StatsCollector.createTask → retrieveStats → Message.StatsEvent → sendMessage
        ├─ sendMessage(Message.ReadyEvent(false, sessionId))
        └─ eventEmitter.onWebSocketOpen(false)

REST PATCH /v4/sessions/{sid}/players/{gid} (PlayerRestHandler.patchPlayer)
  ├─ socketContext(socketServer, sessionId)  → SocketContext (or 404)
  ├─ existingPlayer(context, guildId)         → LavalinkPlayer (or 404)
  ├─ voice update → koe.destroyConnection + getMediaConnection(player).connect(VoiceServerInfo)
  ├─ pause / volume / position / endTime / userData applied to player
  ├─ filters → FilterChain.parse(filters, filterExtensions) → player.filters
  ├─ encodedTrack → decodeTrack(audioPlayerManager, it)   (Base64 → MessageInput → decodeTrack)
  ├─ identifier   → loadAudioItem(manager, identifier)    (loadItemSync → FriendlyException wrap)
  │     └─ must be AudioTrack (else 400 "Cannot play a playlist or search result")
  ├─ player.play(newTrack) → audioPlayer.playTrack(track) + sendPlayerUpdate
  │     └─ LavalinkPlayer.onTrackStart → schedule playerUpdateService (fixedRate, playerUpdateInterval s)
  └─ return player.toPlayer(context, pluginInfoModifiers)

REST GET /v4/loadtracks (AudioLoaderRestHandler.loadTracks)
  ├─ loadAudioItem(audioPlayerManager, identifier)
  ├─ null → NoMatches
  ├─ AudioTrack → TrackLoaded(track.toTrack(...))  — encodeTrack: MessageOutput → Base64
  ├─ AudioPlaylist → PlaylistLoaded / SearchResult
  └─ FriendlyException → LoadFailed

WebSocket → server→client only (handleTextMessage logs "not supported")
  Ops: ready, playerUpdate, stats, event (TrackStart/End/Exception/Stuck/WebSocketClosed)
```

---

## 5. Configuration surface (OBSERVED from `ServerConfig.kt`)

**`application.yml`** (`$R/BOOT-INF/classes/application.yml`, 279 bytes) is minimal; most keys come from `lavalink.server.*` binding.

```yaml
spring:
  application:
    version: "1.0"
    formatted-version: "(v1.0)"
    name: "Lavalink"
  cloud:
    config:
      enabled: false
server:
  error:
    include-message: ALWAYS
    include-binding-errors: ALWAYS
    include-stacktrace: ON_PARAM
    include-exception: false
```

### `lavalink.server` keys

| Key | Type | Default | Label |
|-----|------|---------|-------|
| `password` | String | null | OBSERVED |
| `isNonAllocatingFrameBuffer` | bool | false | OBSERVED |
| `bufferDurationMs` | int | null | OBSERVED |
| `frameBufferDurationMs` | int | null | OBSERVED |
| `opusEncodingQuality` | int | null | OBSERVED |
| `resamplingQuality` | enum (`ResamplingQuality`) | null | OBSERVED |
| `trackStuckThresholdMs` | long | null | OBSERVED |
| `useSeekGhosting` | bool | null | OBSERVED |
| `youtubePlaylistLoadLimit` | int | null | OBSERVED |
| `playerUpdateInterval` | int | 5 (seconds) | OBSERVED |
| `isGcWarnings` | bool | true | OBSERVED |
| `isYoutubeSearchEnabled` | bool | true | OBSERVED |
| `isSoundcloudSearchEnabled` | bool | true | OBSERVED |
| `soundcloudFilterOutPreviewTracks` | bool | false | OBSERVED |
| `ratelimit` | object | null | OBSERVED |
| `youtubeConfig` | object | null | OBSERVED |
| `httpConfig` | object | null | OBSERVED |
| `filters` | Map<String,bool> | empty | OBSERVED |
| `timeouts` | object | null | OBSERVED |
| `timeouts.connectTimeoutMs` | int | 3000 | OBSERVED |
| `timeouts.connectionRequestTimeoutMs` | int | 3000 | OBSERVED |
| `timeouts.socketTimeoutMs` | int | 3000 | OBSERVED |

### `lavalink.plugins[*]` (from `PluginsConfig` + `PluginManager`)

| Key | Type | Default | Label |
|-----|------|---------|-------|
| `dependency` | String (`group:name:version`) | required | OBSERVED |
| `repository` | String | `https://maven.lavalink.dev/releases` | INFERRED from name |
| `snapshot` | bool | false | INFERRED from name |
| `defaultPluginRepository` / `defaultPluginSnapshotRepository` | String | — | INFERRED |
| `pluginsDir` | String | `./plugins` | INFERRED |

---

## 6. Auth model (OBSERVED from `RequestAuthorizationFilter.kt`)

`RequestAuthorizationFilter` implements `HandlerInterceptor` + `WebMvcConfigurer`, registered globally via `addInterceptors`.

- `preHandle(request, response, handler)`:
  - Metrics endpoint (`metricsConfig.endpoint`) → **anonymous** (allowed).
  - `/error` → allowed.
  - Else: read `Authorization` header; compare to `serverConfig.password`:
    - missing → `401 UNAUTHORIZED`, log warning.
    - mismatch → `403 FORBIDDEN`, log warning.
    - match → proceed (`true`).
- Auth is **password/bearer-style** via the `Authorization` header on all `/v4` REST routes.
- WebSocket path `/v4/websocket` does **not** apply this servlet interceptor (it is a separate WS handler); auth for WS is inferred to be via the same `Authorization` header checked at handshake (docs), but the servlet filter does not cover WS upgrade.

**Auth model summary:** single shared password, compared with `==` (not constant-time — static observation), gating all REST mutation/query routes except metrics and error. No per-user/per-token scopes observed.

---

## 7. Session model (OBSERVED from `SocketServer.kt` / `SocketContext.kt`)

- A **session** = 16-char alphanumeric `[a-z0-9]` `sessionId`, generated via `charPool.random()` with uniqueness check against `sessions`.
- Created on WS connect (`afterConnectionEstablished`) with headers: `Authorization`, `User-Id` (required, parsed to `Long`), `Client-Name` (optional), `User-Agent` (optional), optional `Session-Id` for resume.
- `SocketServer.sessions: ConcurrentHashMap<String, SocketContext>` — active sessions.
- `SocketServer.resumableSessions: mutableMapOf<String, SocketContext>` — paused/resumable sessions.
- On WS close: if `context.resumable` → move to `resumableSessions`, `context.pause()`, schedule `onSessionResumeTimeout` after `resumeTimeout` (default **60 seconds**). On timeout → `context.shutdown()`.
- Resume: remove from `resumableSessions`, re-add to `sessions`, `context.resume(session)` replays queued events and sends `ReadyEvent(true, sessionId)` + `sendPlayerUpdate` per player.
- `SocketContext` holds: `players: ConcurrentHashMap<Long, LavalinkPlayer>`, `eventEmitter`, `koe: KoeClient` (constructed via `koe.newClient(userId)`), `sessionPaused` flag, `resumeTimeout`, `executor` (single-thread scheduled), `playerUpdateService` (2-thread scheduled), `resumeEventQueue` (ConcurrentLinkedQueue).
- Session state enum: `OPEN` / `RESUMABLE` / `DESTROYED`.
- `shutdown()`: shuts down executors, destroys all players, closes koe, emits `onSocketContextDestroyed`.

**Session model summary:** sessionId scopes players (keyed by guildId); resume within default 60s timeout; paused sessions queue outbound events; shutdown tears down players + voice connections.

---

## 8. Player model (OBSERVED from `LavalinkPlayer.kt` / `FilterChain.kt` / `EventEmitter.kt` / util)

- A **player** = per `guildId` (`Long`) inside a `SocketContext`, created lazily via `context.getPlayer(guildId)` → `ConcurrentHashMap<Long, LavalinkPlayer>`.
- `LavalinkPlayer` implements `AudioEventAdapter()` + `IPlayer`:
  - `audioPlayer: AudioPlayer` (Lavaplayer) created via `audioPlayerManager.createPlayer()`, with listeners: itself, `EventEmitter(...)`, `AudioLossCounter`.
  - `filters: FilterChain` — settable; setter applies `audioPlayer.setFilterFactory(value.takeIf { it.isEnabled })`.
  - `audioLossCounter: AudioLossCounter` — tracks frames sent/nulled per minute (expected 3000 frames/min/player = one per 20 ms).
  - `updateFuture: ScheduledFuture<*>` — periodic `sendPlayerUpdate` scheduled on `onTrackStart`, cancelled on `onTrackEnd`.
  - `Provider : AudioFrameProvider` — pulls PCM from `audioPlayer.provide(mutableFrame)` into a `ByteBuffer`, writes to Discord `ByteBuf` via `koe` media connection.
- `play(track)`: `audioPlayer.playTrack(track)` + `sendPlayerUpdate`.
- `stop()`: `audioPlayer.stopTrack()`.
- `setPause(pause)`, `seekTo(position)`, `setVolume(volume)`.

### Filter chain (OBSERVED exact order — `FilterChain.buildList()`)

```
1. volume
2. equalizer   (15 bands: 25 Hz … 16 kHz, gain −0.25 … 1.0)
3. karaoke
4. timescale   (speed/pitch/rate)
5. tremolo
6. vibrato
7. distortion
8. rotation
9. channelMix
10. lowPass
11. pluginFilters (last, from AudioFilterExtension beans)
```

`FilterChain.parse(filters, extensions)` builds the chain from wire `Filters`; `buildChain(track, format, output)` constructs the `MutableList<AudioFilter>` pipeline (output last via `.reversed()`).

### Encoded-track format (OBSERVED mechanism, INFERRED byte layout)

`encodeTrack(audioPlayerManager, track)`:
1. `ByteArrayOutputStream` → `MessageOutput(baos)` → `audioPlayerManager.encodeTrack(output, track)` → Lavaplayer's internal serialized `AudioTrack`.
2. `Base64.getEncoder().encodeToString(...)` → the `encoded` string in every API payload.

`decodeTrack`: `Base64.getDecoder().decode(message)` → `ByteArrayInputStream` → `MessageInput(bais)` → `audioPlayerManager.decodeTrack(input).decodedTrack`.

**INFERRED:** The `MessageOutput`/`MessageInput` binary layout is Lavaplayer-private (track identifier + `AudioTrackInfo` + source-specific decoded data + position). Not decompiled here (REA archive providers reject ZIP; only `extract-artifact` + `javap` used). For protocol compatibility NEXORA must define its own binary track container — semantically equivalent (identifier + info + opaque source payload), not byte-compatible with Lavaplayer.

### Event pipeline (OBSERVED `EventEmitter.kt` per player)

Lavaplayer `AudioEventAdapter` → WS `event` op:
- `onTrackStart` → `TrackStartEvent` (guildId, track)
- `onTrackEnd` → `TrackEndEvent` (guildId, track, reason: FINISHED/LOAD_FAILED/STOPPED/REPLACED/CLEANUP); `endMarkerHit` special-case forces FINISHED
- `onTrackException` → `TrackExceptionEvent` (guildId, track, Exception{message, severity, cause, causeStackTrace}); root cause extracted via `getRootCause`
- `onTrackStuck` → `TrackStuckEvent` (guildId, track, thresholdMs) + `sendPlayerUpdate`
- Koe `WsEventHandler` → `WebSocketClosedEvent` (guildId, code, reason, byRemote) on `gatewayClosed`; `sendPlayerUpdate` on `gatewayReady`/`gatewayError`.

### Player → wire shape (`Player`)

```
Player {
  guildId: String,
  track: Track?,            // {encoded, info{identifier,isSeekable,author,length,isStream,position,title,uri,sourceName,artworkUrl,isrc}, pluginInfo{}, userData{}}
  volume: Int,
  paused: Boolean,
  state: PlayerState {time, position, connected, ping},
  voice: VoiceState {token, endpoint, sessionId, channelId?},
  filters: Filters {volume, equalizer[karaoke...], ..., pluginFilters: Map<String,JsonElement>}
}
```

---

## 9. REST surface (`/v4/...`, password via `Authorization` header)

| Endpoint | Method | Purpose | Label |
|----------|--------|---------|-------|
| `/v4/loadtracks?identifier=` | GET | resolve track/playlist/search → LoadResult (track|playlist|search|empty|error) | OBSERVED |
| `/v4/decodetrack?encodedTrack=` | GET | decode one track → Track | OBSERVED |
| `/v4/decodetracks` | POST | decode array of tracks → Tracks | OBSERVED |
| `/v4/sessions/{sessionId}/players` | GET | list players in session | OBSERVED |
| `/v4/sessions/{sessionId}/players/{guildId}` | GET | get player | OBSERVED |
| `/v4/sessions/{sessionId}/players/{guildId}?noReplace=` | PATCH | update/create player | OBSERVED |
| `/v4/sessions/{sessionId}/players/{guildId}` | DELETE | destroy player | OBSERVED |
| `/v4/sessions/{sessionId}` | PATCH | update session (resuming/timeout) | OBSERVED |
| `/v4/info` | GET | version/build/git/jvm/lavaplayer/sourceManagers/filters/plugins | OBSERVED |
| `/version` | GET | plain version string | OBSERVED |
| `/v4/stats` | GET | players/playingPlayers/uptime/memory/cpu | OBSERVED |
| `/v4/routeplanner/status` | GET | IP route planner status | OBSERVED |
| `/v4/routeplanner/free/address` | POST | unmark failed address | OBSERVED |
| `/v4/routeplanner/free/all` | POST | unmark all failed addresses | OBSERVED |

`LoadResult` wire shape: `loadType` ∈ {track, playlist, search, empty, error} + `data` (Track | Playlist{info, pluginInfo, tracks} | SearchResult.Data{tracks} | null | Exception{message,severity,cause,causeStackTrace}).

---

## 10. WebSocket surface (`/v4/websocket`)

Headers: `Authorization`, `User-Id`, `Client-Name`, optional `Session-Id`.

Server→client ops (all `Message` with `op`):
| op | Meaning | Label |
|----|---------|-------|
| `ready` | session id + resumed flag | OBSERVED |
| `playerUpdate` | periodic player state (time, position, connected, ping) | OBSERVED |
| `stats` | per-minute node stats (memory, cpu, frameStats) | OBSERVED |
| `event` | TrackStart/End/Exception/Stuck/WebSocketClosed | OBSERVED |

Client→server WS messages: **not supported in v4** — `handleTextMessage` logs "Lavalink v4 does not support websocket messages. Please use the REST api." (OBSERVED).

---

## 11. Plugin loading mechanism (OBSERVED from `PluginManager.kt`)

1. `manageDownloads()` reads `config.plugins` declarations (`group:name:version`).
2. For each declaration: checks `pluginsDir/*.jar` for matching version; if missing, downloads from `{repository}{group}/{name}/{version}/{name}-{version}.jar`; deletes stale versions; checks `maven-metadata.xml` for updates.
3. `readClasspathManifests()` scans `classpath*:lavalink-plugins/*.properties`.
4. `loadJars()` builds a child `URLClassLoader` from plugin jars, loads all `.class` entries under allowed package paths (from manifest `path`), and parses `lavalink-plugins/*.properties` manifests (`name`, `path`, `version`).
5. Manifests merged into `pluginManifests`, driving `componentScan` paths in the main context — plugin `@Service` beans auto-registered.

**Plugin extension points** (`dev.arbjerg.lavalink.api`):
- `AudioSourceManager` — add a source
- `MediaContainerProbe` — HTTP/local container probe
- `RestInterceptor` — intercept/modify REST endpoints
- `AudioPluginInfoModifier` — add custom info to track/playlist JSON
- `AudioFilterExtension` — custom DSP filter (used by `FilterChain.parsePluginConfigs`)
- `IPlayer`, `ISocketContext`, `ISocketServer`, `PluginEventHandler` — SPI interfaces

---

## 12. OBSERVED vs INFERRED separation

**OBSERVED** (directly verified from extracted JAR + source):
- JAR SHA-256, version 4.2.2, build timestamp, 328 zip entries, 98 lavalink classes, 132 `BOOT-INF/lib` jars with exact versions.
- `application.yml` content (minimal; most config via `lavalink.server.*` binding).
- `ServerConfig` fields and defaults (exact names/defaults read from source).
- `SocketServer` WebSocket path `/v4/websocket`, required headers, resume logic, `handleTextMessage` unsupported note.
- `AudioLoaderRestHandler` exact endpoint signatures and query params.
- `LavalinkPlayer` structure: `AudioPlayer`, `FilterChain`, `MutableAudioFrame` provider, `ScheduledFuture` update loop.
- `FilterChain.buildList()` exact order: volume → equalizer → karaoke → timescale → tremolo → vibrato → distortion → rotation → channelMix → lowPass → pluginFilters.
- `encodeTrack`/`decodeTrack` via Lavaplayer `MessageOutput`/`MessageInput` + Base64 (util.kt lines 131–141).
- `PluginManager` download + `URLClassLoader` + manifest properties flow.
- Event types: TrackStart, TrackEnd, TrackException, TrackStuck, WebSocketClosed (docs + EventEmitter).
- Track Info fields: identifier, isSeekable, author, length, isStream, position, title, uri, sourceName, artworkUrl, isrc (docs + `toInfo()`).
- Auth: `RequestAuthorizationFilter` compares `Authorization` header == `serverConfig.password`; 401/403; metrics + `/error` anonymous.
- Session: 16-char alnum sessionId, `sessions` + `resumableSessions` maps, default 60s resumeTimeout, pause/queue/resume/shutdown.

**INFERRED** (not directly proven by static analysis):
- Lavaplayer internal binary track format (`MessageOutput`/`MessageInput` byte layout) — not decompiled here.
- Exact `koe` (Discord voice gateway) behaviour — class present but internals not inspected.
- Runtime thread-pool sizes beyond observed (`executor` single-thread, `playerUpdateService` 2-thread), frame pacing, resume-timeout default values (60s observed in source).
- Whether all 132 jars are actually loaded at runtime vs bundled-but-unused.
- Exact rate-limit strategy implementation details.
- WS `Authorization` header check at handshake (inferred from docs; servlet filter doesn't cover WS upgrade).

---

## 13. Limitations and unknowns

1. `rea inspect` (binary-analysis providers: Ghidra/Hopper/IDA/Kuna) rejects ZIP/archive targets — only `extract-artifact` + shell `unzip`/`javap` worked. No bytecode decompilation of Lavaplayer internals was achieved.
2. Lavaplayer source is NOT inside this JAR's server classes; it is a separate dependency jar. Its `encodeTrack`/`decodeTrack` byte format and `AudioSourceManager` implementations were not reverse-engineered here.
3. Static analysis cannot confirm runtime behaviour (thread scheduling, frame pacing, actual dependency loading, network timing, GC). All runtime claims are INFERRED.
4. REA extraction limitation: "nested archive contents remain represented by their containing file" — inner jars' classes were extracted separately for `javap` but REA did not recurse into them for analysis.
5. Plugin `@Service` bean registration and `AudioFilterExtension` contract inferred from docs + imports, not from decompiled plugin bytecode.
6. gbrain code traversal not built for this source tree (`not_built`); call graph above derived from manual source reads, not from gbrain `code_flow`/`code_callers`.

---

## 14. Evidence trail

| Step | Command / source | Output file | Result |
|------|------------------|-------------|--------|
| Download + hash verify | `sha256sum Lavalink.jar` | — | SHA-256 matched `8cb801e5...` |
| REA bind | `mcp__gbrain__code_flow` | — | `not_built` (gbrain not built for this tree) |
| REA archive extraction | `rea extract-artifact ... --integrity-policy record-and-continue --format json` | `rea-extract.json` | 304 artifacts, containment verified |
| Shell extraction | `unzip -q -o Lavalink.jar 'BOOT-INF/classes/*' 'BOOT-INF/lib/*.jar' -d extracted` | `$R` | 304 files materialized |
| `application.yml` | `cat $R/BOOT-INF/classes/application.yml` | inline above | minimal config |
| `app.properties` | `cat $R/BOOT-INF/classes/app.properties` | inline above | version 4.2.2 |
| Class inventory | `find $R/BOOT-INF/classes/lavalink -name "*.class"` | `class-inventory.txt` | 98 classes, package map |
| Dependency list | `ls $R/BOOT-INF/lib/` | `lib-list.txt` | 132 jars with versions |
| Source read (Launcher) | `lavalink-src/.../Launcher.kt` | — | two-phase Spring Boot entry |
| Source read (SocketServer) | `.../io/SocketServer.kt` | — | WS/session lifecycle |
| Source read (SocketContext) | `.../io/SocketContext.kt` | — | per-session state, players map, resume |
| Source read (LavalinkPlayer) | `.../player/LavalinkPlayer.kt` | — | player/audio path |
| Source read (FilterChain) | `.../player/filters/FilterChain.kt` | — | filter order + plugin filters |
| Source read (AudioLoaderRestHandler) | `.../player/AudioLoaderRestHandler.kt` | — | /v4/loadtracks + decode |
| Source read (PlayerRestHandler) | `.../player/PlayerRestHandler.kt` | — | /v4/sessions/{sid}/players/{gid} PATCH |
| Source read (util.kt) | `.../util/util.kt` | lines 125–141 | encode/decode track |
| Source read (RequestAuthorizationFilter) | `.../config/RequestAuthorizationFilter.kt` | — | auth model |
| Source read (EventEmitter player) | `.../player/EventEmitter.kt` | — | event pipeline |
| Source read (ServerConfig) | `.../config/ServerConfig.kt` | — | config keys/defaults |
| Source read (PluginManager) | `.../bootstrap/PluginManager.kt` | — | plugin loading mechanism |
| Source read (StatsCollector) | `.../io/StatsCollector.kt` | — | /v4/stats + frameStats |
| Protocol messages | `protocol/.../messages.kt` | — | Message ops, events |
| Protocol loadResult | `protocol/.../loadResult.kt` | — | LoadResult wire shape |
| Protocol player | `protocol/.../player.kt` | — | Player/Track/TrackInfo/VoiceState/PlayerState |
| Protocol filters | `protocol/.../filters.kt` | — | Filters wire shape |
| Protocol session | `protocol/.../session.kt` | — | Session/SessionUpdate |
| Protocol stats | `protocol/.../stats.kt` | — | Stats/FrameStats/Memory/Cpu |
| Official REST docs | `curl lavalink.dev/api/rest.html` | `lavalink-docs/api_rest.txt` | full REST surface |
| Official WS docs | `curl lavalink.dev/api/websocket.html` | `lavalink-docs/api_websocket.txt` | full WS event surface |

---

## 15. Most important findings for a Rust reimplementation

1. **Audio pipeline is the whole product.** Lavalink = Lavaplayer (decode/playback) + Spring (REST+WS framing) + koe (Discord voice). A from-scratch Rust system must replace all three: a custom audio pipeline (source → demux → decode → filter → resample → opus-encode → UDP-send), an HTTP+WS server (axum + tokio-tungstenite), and a Discord voice-gateway client.
2. **Filter chain is fixed and ordered.** 10 built-in filters + plugin filters, applied in a specific order (volume first, lowPass last, plugin filters last). Rust should mirror this exact order for protocol compatibility.
3. **Encoded track is an opaque Base64 blob.** Produced by Lavaplayer `encodeTrack` → `MessageOutput` → Base64. Rust needs its own binary track container (identifier + info + seekable + position + stream flag + source-specific payload). Protocol compatibility requires defining this container explicitly.
4. **Sessions are the scoping unit.** A `sessionId` (16-char alnum) groups players; players are keyed by `guildId`; resume within timeout (default 60s). Rust should model `Session { players: HashMap<guildId, Player> }` with resume/timeout.
5. **Plugin model is classpath-based jar loading.** Rust equivalent: dynamic library loading or a WASM/plugin ABI, not classpath scanning. Keep the extension points (source manager, filter, REST interceptor, info modifier) but implement via Rust traits + `libloading`/`wasmtime`.
6. **Zero third-party runtime dependency means:** no JVM, no Spring Boot, no Lavaplayer. Native Rust audio codec crates (opus, mp3, flac decoders) are acceptable as *libraries*, since the user's constraint is about runtime dependency, not about avoiding all libraries.

---

## 16. Workflow

Use the rust-anything discipline: this document records the original's observable behavior; change one unit at a time and prove parity with contract tests before touching protocol-facing code. Re-run REA inspection when a new Lavalink release changes the shipped artifact.

**Parity units completed (NEXORA side, per `docs/LAVALINK_ANALYSIS.md`):**
- `/v4/loadtracks` returns v4 lowercase `loadType` values (`empty` for no results, `search` for results); `/v4/stats` uses `cpu.lavalinkLoad` instead of `cpu.processLoad`.
- `/v4/loadtracks` maps every result to the v4 track shape: `encoded`, `info`, `pluginInfo`, `userData`. The `encoded` string is an opaque NEXORA base64 JSON blob, not Lavalink's binary track format.
- `/v4/info` has the documented `version` object, numeric `buildTime` and `git.commitTime`, and real `sourceManagers` from the registry. `/v4` player responses carry `state.time`, `state.connected`, `state.ping` and `filters`; `POST /v4/sessions` returns 201.

**Remaining gap:** Lavalink binary track encoding and `/v4/decodetrack` are not implemented. That is a material compatibility boundary, not an accident.

---

*End of REA static analysis.*
