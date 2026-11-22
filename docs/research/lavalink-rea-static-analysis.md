# Lavalink v4.2.2 — REA Static Analysis (JAR)

**Target artifact:** `/home/nickfury6023/.jcode/scratch/lavalink-re/Lavalink.jar`
**SHA-256:** `8cb801e591072c3689fafd71ccf571a95a4ead3cc35dfc045e157d763d89119a` (matches `docs/LAVALINK_ANALYSIS.md` evidence `ev_80dae1e96851ebd5550a80e9ba753eef7b603d1a2c0a81b0395cdfced5a1d3d6`)
**Format:** Spring Boot fat JAR = ZIP archive (~100 MB, 328 zip entries)
**Extracted via:** `rea extract-artifact Lavalink.jar --integrity-policy record-and-continue --format json` → `rea-extract.json` (304 artifacts, containment verified, no integrity contradictions)
**Materialized output root:** `/home/nickfury6023/.jcode/scratch/rea-extracted-9740d6c8-9749-4c77-9b71-f810dab830fe` (`$R`)
**Build time:** `1772839456827` (from `BOOT-INF/classes/app.properties`) → version `4.2.2`
**Class file target:** JVM 17 (`kotlin.jvmTarget = JVM_17` per `build.gradle.kts`); Kotlin 2.1.20
**Not executed:** Static analysis only. No dynamic/runtime claims proven.

**Constraint labels used below:**
- **OBSERVED:** directly seen in extracted bytes/classes/properties/docs
- **DERIVED:** computed from observed data (e.g. package counts, version aggregates)
- **INFERRED:** reasoning about likely behaviour; static analysis does not prove runtime behaviour

---

## 1. Artifact identity

| Field | Value | Label |
|-------|-------|-------|
| SHA-256 | `8cb801e591072c3689fafd71ccf571a95a4ead3cc35dfc045e157d763d89119a` | OBSERVED |
| Format | ZIP / Spring Boot fat JAR | OBSERVED |
| Zip entries | 328 | OBSERVED |
| Version | 4.2.2 (`app.properties`) | OBSERVED |
| Build timestamp | `1772839456827` ms epoch | OBSERVED |
| Group / artifact | `dev.arbjerg.lavalink` / `Lavalink-Server` | OBSERVED |
| JVM target | JVM 17 | DERIVED (from build config) |
| Kotlin | 2.1.20 | DERIVED (from build config) |
| REA extraction artifacts | 304 materialized files | OBSERVED |
| REA containment verified | true | OBSERVED |
| Integrity contradictions | none | OBSERVED |

---

## 2. Package / class inventory (lavalink/server)

Derived from `$R/BOOT-INF/classes/lavalink/` (98 class files) — recursive directory listing:

| Package | Class count | Sub-packages |
|---------|-------------|--------------|
| `lavalink/server` | 2 | — |
| `lavalink/server/bootstrap` | 17 | — |
| `lavalink/server/config` | 18 | — |
| `lavalink/server/info` | 5 | — |
| `lavalink/server/io` | 21 | — |
| `lavalink/server/metrics` | 5 | — |
| `lavalink/server/player` | 12 | `filters` (15) |
| `lavalink/server/util` | 3 | — |

**Key classes (observed from source mirror at `/home/nickfury6023/.jcode/scratch/lavalink-src/`):**

| File | Responsibility | Label |
|------|----------------|-------|
| `LavalinkServer/src/main/java/lavalink/server/Launcher.kt` | Entry point; two-phase Spring Boot: plugin bootstrap (no web) then main web app | OBSERVED |
| `.../LavalinkApplication.kt` | `@SpringBootApplication` + component scan | OBSERVED |
| `.../bootstrap/PluginManager.kt` | Downloads plugin jars from maven, verifies manifests, loads into `URLClassLoader` | OBSERVED |
| `.../bootstrap/PluginManifest.kt` | `lavalink-plugins/*.properties` (name, path, version) | OBSERVED |
| `.../bootstrap/PluginsConfig.kt` | `lavalink.plugins[*]` declarations (dependency/repository/snapshot) | INFERRED from name |
| `.../config/ServerConfig.kt` | `lavalink.server.*` binding (password, buffers, opus, resampling, filters, ratelimit, youtube, http, timeouts) | OBSERVED |
| `.../config/AudioPlayerConfiguration.kt` | Lavaplayer `AudioPlayerManager` bean wiring | INFERRED from name |
| `.../config/AudioSourcesConfig.kt` | Source-manager enablement flags | INFERRED from name |
| `.../io/SocketServer.kt` | WebSocket `/v4/websocket`; session lifecycle (create/resume/close/pause); `sendPlayerUpdate` | OBSERVED |
| `.../io/SocketContext.kt` | Per-session state: players map, media connection, voice server info, event emitter, player update scheduler | OBSERVED |
| `.../io/SocketContext.kt:149` | `send(json.encodeToString(serializer, message))` — WS frame serialization | OBSERVED |
| `.../player/LavalinkPlayer.kt` | Guild player: Lavaplayer `AudioPlayer`, `FilterChain`, `MutableAudioFrame` provider, `AudioFrameProvider` | OBSERVED |
| `.../player/EventEmitter.kt` | `AudioEventAdapter` → WS events (TrackStart/End/Exception/Stuck) | OBSERVED |
| `.../player/PlayerRestHandler.kt` | `PATCH /v4/sessions/{sessionId}/players/{guildId}` — update/create/destroy player | INFERRED from docs |
| `.../player/AudioLoaderRestHandler.kt` | `GET /v4/loadtracks`, `/v4/decodetrack`, `/v4/decodetracks` | OBSERVED |
| `.../player/filters/FilterChain.kt` | `PcmFilterFactory`; ordered filter list; plugin filter extensibility | OBSERVED |
| `.../player/filters/filterConfigs.kt` | Per-effect config (volume/equalizer/karaoke/timescale/tremolo/vibrato/distortion/rotation/channelMix/lowPass) | INFERRED from docs |
| `.../util/util.kt` | `encodeTrack`/`decodeTrack` via Lavaplayer `MessageOutput`/`MessageInput` + Base64 | OBSERVED |
| `.../util/loading.kt` | `loadAudioItem` → `AudioPlayerManager.loadItemSync` + `FriendlyException` wrapping | OBSERVED |
| `.../metrics/StatsCollector.kt` | node stats (players, playingPlayers, uptime, memory, cpu, frameStats) | INFERRED from docs |
| `.../info/AppInfo.kt` | version/build/git info for `/v4/info` | INFERRED from name |

**Not present / not shipped:** `lavalink/server/audio/` source-manager implementation classes are NOT in this server module — they live in Lavaplayer (`lavaplayer-2.2.6.jar`) under `com.sedmelluq.discord.lavaplayer.source.*`. The server only wires `AudioPlayerManager`.

---

## 3. Runtime dependency inventory

From `$R/BOOT-INF/lib/` — 132 jar files. Grouped by what they provide and what a Rust replacement would need.

### Core framework (would NOT be reused in Rust; Rust is from-scratch)

| Jar | Version | Provides | Rust replacement candidate | Label |
|-----|---------|----------|---------------------------|-------|
| `spring-boot` | 3.3.0 | app framework | — (Rust std + custom) | OBSERVED |
| `spring-boot-autoconfigure` | 3.3.0 | autoconfig | — | OBSERVED |
| `spring-webmvc` | 6.1.8 | REST controllers | axum / actix-web | INFERRED |
| `spring-websocket` | 6.1.9 | WS server | tokio-tungstenite | INFERRED |
| `tomcat-embed-el` | 10.1.24 | embedded servlet | — (custom HTTP) | INFERRED |
| `undertow-core` | 2.3.13 | alt servlet/WS container | — | OBSERVED |
| `jackson-*` | 2.17.1 | JSON (de)serialization | serde_json | INFERRED |
| `kotlin-stdlib` / `kotlin-reflect` | 2.1.20 | Kotlin runtime | — (Rust std) | OBSERVED |
| `kotlinx-serialization-*` | 1.7.3 | Kotlin serialization | serde | INFERRED |
| `logback-*` / `log4j-*` / `slf4j-api` | 1.5.6 / 2.23.1 / 2.0.17 | logging | tracing / log | INFERRED |
| `micrometer-*` | 1.12.12 | metrics | custom metrics | INFERRED |
| `sentry-*` | 7.10.0 | error reporting | — (optional) | INFERRED |

### Audio / DSP (the heart of what Rust must reimplement)

| Jar | Version | Provides | Rust replacement candidate | Label |
|-----|---------|----------|---------------------------|-------|
| `lavaplayer` | 2.2.6 | track loading, decoding, playback engine | **custom audio pipeline** (source → demux → decode → filter → resample → encode) | OBSERVED |
| `lavadsp` | 0.7.8 | audio DSP filters | **custom DSP** (volume, EQ, karaoke, timescale, tremolo, vibrato, distortion, rotation, channelMix, lowPass) | OBSERVED |
| `lava-common` | 2.2.6 | lavaplayer common | **custom** | INFERRED |
| `lavaplayer-natives` | 2.2.6 | native codecs (opus, mp3, flac, vorbis, etc.) | **native codec crates** (rodio/ Symphonia / custom) | OBSERVED |
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

**Key takeaway for Rust:** Lavalink's runtime value is ~80% Lavaplayer (audio pipeline) + spring (web/WS framing) + koe (Discord voice). A from-scratch Rust system replaces Spring with axum/tokio-tungstenite, Lavaplayer with a custom audio pipeline, koe with a custom voice-gateway client, and keeps only std + serde + a crypto/audio codec crate set. Zero third-party *runtime* dependency in the user's sense means: no JVM, no Spring, no Lavaplayer — but audio codecs (opus, etc.) are acceptable as native Rust crates since they are libraries, not an external runtime service.

---

## 4. Configuration surface

Embedded `BOOT-INF/classes/application.yml` (`$R/BOOT-INF/classes/application.yml`, 279 bytes) is minimal — most keys come from `lavalink.server.*` binding in `ServerConfig` (observed in source):

```
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

**`lavalink.server` keys (observed from `ServerConfig.kt`):**

| Key | Type | Default | Label |
|-----|------|---------|-------|
| `password` | String | null | OBSERVED |
| `isNonAllocatingFrameBuffer` | bool | false | OBSERVED |
| `bufferDurationMs` | int | null | OBSERVED |
| `frameBufferDurationMs` | int | null | OBSERVED |
| `opusEncodingQuality` | int | null | OBSERVED |
| `resamplingQuality` | enum | null | OBSERVED |
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

**`lavalink.plugins[*]` (from `PluginsConfig` + docs):**
- `dependency` (required, `group:name:version`)
- `repository` (optional, default `https://maven.lavalink.dev/releases`)
- `snapshot` (bool, default false)
- `lavalink.defaultPluginRepository` / `defaultPluginSnapshotRepository`
- `lavalink.pluginsDir` (default `./plugins`)

---

## 5. Entry point wiring

**`Launcher.main`** (observed source):
1. `launchPluginBootstrap(args)` — runs `PluginManager` as a separate Spring context with `WebApplicationType.NONE`, banner off. This downloads/verifies/loads plugin jars into a `URLClassLoader` and builds `pluginManifests` (classpath `lavalink-plugins/*.properties` + plugin dir jars).
2. `launchMain(parent, args)` — runs `LavalinkApplication` as child context of the plugin context:
   - `componentScan` = plugin manifest paths + `lavalink.server`
   - `resourceLoader` uses `pluginManager.classLoader` (so plugin classes are resolvable)
   - Listeners log version info on env-prepared, "ready" on ready, error on failed.

**Bean wiring:** `SocketServer` (WebSocket handler), `AudioLoaderRestHandler` (+ `PlayerRestHandler`), `StatsCollector`, `FilterChain`, `EventEmitter` (per player), `LavalinkPlayer` (per guild) are all Spring-managed singletons/prototypes wired via constructor injection.

---

## 6. Handler signatures

### REST (`/v4/...`, password via `Authorization` header)

| Endpoint | Method | Purpose | Label |
|----------|--------|---------|-------|
| `/v4/loadtracks?identifier=` | GET | resolve track/playlist/search | OBSERVED (source + docs) |
| `/v4/decodetrack?encodedTrack=` | GET | decode one track to info | OBSERVED |
| `/v4/decodetracks` | POST | decode array of tracks | OBSERVED |
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

### WebSocket (`/v4/websocket`, headers: `Authorization`, `User-Id`, `Client-Name`, optional `Session-Id`)

| op | Direction | Meaning | Label |
|----|-----------|---------|-------|
| `ready` | server→client | session id + resumed flag | OBSERVED |
| `playerUpdate` | server→client | periodic player state (time, position, connected, ping) | OBSERVED |
| `stats` | server→client | per-minute node stats | OBSERVED |
| `event` | server→client | TrackStart/End/Exception/Stuck/WebSocketClosed | OBSERVED |

Client→server WS messages: **not supported in v4** — `handleTextMessage` logs "Lavalink v4 does not support websocket messages. Please use the REST api." (OBSERVED).

---

## 7. Plugin loading mechanism

Observed in `PluginManager.kt`:
1. `manageDownloads()` reads `config.plugins` declarations (`group:name:version`).
2. For each declaration: checks `pluginsDir/*.jar` for matching version; if missing, downloads from `{repository}{group}/.{name}/{version}/{name}-{version}.jar`.
3. `readClasspathManifests()` scans `classpath*:lavalink-plugins/*.properties`.
4. `loadJars()` builds a child `URLClassLoader` from plugin jars, loads all `.class` entries under allowed package paths, and parses `lavalink-plugins/*.properties` manifests (`name`, `path`, `version`).
5. Manifests are merged into `pluginManifests`, which drive `componentScan` paths in the main context — so plugin `@Service` beans (e.g. `AudioSourceManager`, `MediaContainerProbe`, `RestInterceptor`, `AudioPluginInfoModifier`, `AudioFilterExtension`) are auto-registered as Spring beans.

**Plugin extension points (observed from plugin-api docs + source imports):**
- `AudioSourceManager` (bean) — add a source
- `MediaContainerProbe` (bean) — HTTP/local container probe
- `RestInterceptor` — intercept/modify REST endpoints
- `AudioPluginInfoModifier` — add custom info to track/playlist JSON
- `AudioFilterExtension` — custom DSP filter (used by `FilterChain.parsePluginConfigs`)

---

## 8. OBSERVED vs INFERRED separation

**OBSERVED** (directly verified from extracted JAR + source):
- JAR SHA-256, version 4.2.2, build timestamp, 328 zip entries, 98 lavalink classes, 132 BOOT-INF/lib jars with exact versions.
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

**INFERRED** (not directly proven by static analysis):
- Lavaplayer internal binary track format (the `MessageOutput`/`MessageInput` byte layout) — not decompiled here; Rust would need its own encoding.
- Exact `koe` (Discord voice gateway) behaviour — class present but internals not inspected.
- Runtime thread-pool sizes, frame pacing, resume-timeout default values (config has `timeouts` but defaults not all read).
- Whether all 132 jars are actually loaded at runtime vs bundled-but-unused.
- Exact rate-limit strategy implementation details.

---

## 9. Limitations and unknowns

1. `rea inspect` (binary-analysis providers: Ghidra/Hopper/IDA/Kuna) rejects ZIP/archive targets — only `extract-artifact` + shell `unzip`/`javap` worked. No bytecode decompilation of Lavaplayer internals was achieved.
2. Lavaplayer source is NOT inside this JAR's server classes; it is a separate dependency jar. Its `encodeTrack`/`decodeTrack` byte format and `AudioSourceManager` implementations were not reverse-engineered here.
3. Static analysis cannot confirm runtime behaviour (thread scheduling, frame pacing, actual dependency loading, network behaviour).
4. REA extraction limitation: "nested archive contents remain represented by their containing file" — inner jars' classes were extracted separately for `javap` but REA did not recurse into them for analysis.
5. Plugin `@Service` bean registration and `AudioFilterExtension` contract inferred from docs + imports, not from decompiled plugin bytecode.

---

## 10. Evidence trail

| Step | Command | Output file | Result |
|------|---------|-------------|--------|
| Download + hash verify | `curl .../4.2.2/Lavalink.jar` + `sha256sum` | `/home/nickfury6023/.jcode/scratch/lavalink-re/Lavalink.jar` | SHA-256 matched `docs/LAVALINK_ANALYSIS.md` |
| REA bind | `mcp__rea__open_binary` (timeout) | — | MCP timeout on ~100 MB JAR |
| REA archive extraction | `rea extract-artifact Lavalink.jar --integrity-policy record-and-continue --format json` | `rea-extract.json` | 304 artifacts, containment verified |
| REA manifest | `rea extract-artifact Lavalink.jar --llms` | (not captured) | — |
| Shell extraction | `unzip -q -o Lavalink.jar 'BOOT-INF/classes/*' 'BOOT-INF/lib/*.jar' -d extracted` | `$R` | 304 files materialized |
| `application.yml` | `cat $R/BOOT-INF/classes/application.yml` | inline above | minimal config |
| `app.properties` | `cat $R/BOOT-INF/classes/app.properties` | inline above | version 4.2.2 |
| Class inventory | `find $R/BOOT-INF/classes/lavalink -name "*.class"` | inline above | 98 classes, package map |
| Dependency list | `ls $R/BOOT-INF/lib/` | inline above | 132 jars with versions |
| Source read (Launcher) | `read LavalinkServer/.../Launcher.kt` | — | entry-point wiring |
| Source read (SocketServer) | `read .../SocketServer.kt` | — | WS/session lifecycle |
| Source read (LavalinkPlayer) | `read .../LavalinkPlayer.kt` | — | player/audio path |
| Source read (FilterChain) | `read .../filters/FilterChain.kt` | — | filter order + plugin filters |
| Source read (AudioLoaderRestHandler) | `read .../player/AudioLoaderRestHandler.kt` | — | /v4/loadtracks + decode |
| Source read (util.kt) | `read .../util/util.kt` | lines 125–141 | encode/decode track |
| Source read (EventEmitter) | `read .../player/EventEmitter.kt` | — | event pipeline |
| Source read (ServerConfig) | `read .../config/ServerConfig.kt` | — | config keys/defaults |
| Source read (PluginManager) | `read .../bootstrap/PluginManager.kt` | — | plugin loading mechanism |
| javap AudioPlayerManager | `javap -p ...AudioPlayerManager.class` | — | encodeTrack/decodeTrack signatures |
| Official REST docs | `curl lavalink.dev/api/rest.html` | `lavalink-docs/api_rest.txt` | full REST surface |
| Official WS docs | `curl lavalink.dev/api/websocket.html` | `lavalink-docs/api_websocket.txt` | full WS event surface |
| Official plugin docs | `curl lavalink.dev/api/plugins.html` | `lavalink-docs/api_plugins.txt` | plugin extension points |
| Official config docs | `curl lavalink.dev/configuration/config/file.html` | `lavalink-docs/configuration_config_file.txt` | env-var config keys |

---

## Most important findings for a Rust reimplementation

1. **Audio pipeline is the whole product.** Lavalink = Lavaplayer (decode/playback) + Spring (REST+WS framing) + koe (Discord voice). A from-scratch Rust system must replace all three: a custom audio pipeline (source → demux → decode → filter → resample → opus-encode → UDP-send), an HTTP+WS server (axum + tokio-tungstenite), and a Discord voice-gateway client.
2. **Filter chain is fixed and ordered.** 10 built-in filters + plugin filters, applied in a specific order (volume first, lowPass last, plugin filters last). Rust should mirror this exact order for protocol compatibility.
3. **Encoded track is an opaque Base64 blob.** Produced by Lavaplayer `encodeTrack` → `MessageOutput` → Base64. Rust needs its own binary track container (identifier + info + seekable + position + stream flag + source-specific payload). Protocol compatibility requires defining this container explicitly.
4. **Sessions are the scoping unit.** A `sessionId` (16-char alnum) groups players; players are keyed by `guildId`; resume within timeout (default 60s). Rust should model `Session { players: HashMap<guildId, Player> }` with resume/timeout.
5. **Plugin model is classpath-based jar loading.** Rust equivalent: dynamic library loading or a WASM/plugin ABI, not classpath scanning. Keep the extension points (source manager, filter, REST interceptor, info modifier) but implement via Rust traits + `libloading`/`wasmtime`.
6. **Zero third-party runtime dependency means:** no JVM, no Spring Boot, no Lavaplayer. Native Rust audio codec crates (opus, mp3, flac decoders) are acceptable as *libraries*, since the user's constraint is about runtime dependency, not about avoiding all libraries.
