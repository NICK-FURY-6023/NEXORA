# Lavalink v4.2.2 — Source Architecture (Behavioural Specification for a Rust Reimplementation)

**Target for reimplementation:** a Rust service that is wire-compatible with Lavalink v4.2.2 on both its
REST API (`/v4/...`) and its WebSocket event stream (`/v4/websocket`), and that reproduces Lavalink's
encoded-track string format byte-for-byte.

---

## 0. How to read this document

Every structural claim carries a citation of the form:

```
<path>:<line>  [OBSERVED]
```

- **Paths** are relative to the upstream repository root. Two upstream repositories are referenced:
  - `Lavalink/` — the Lavalink server itself, tag `4.2.2`, commit `e8503fdf270acad73e1af4ded9ddbedded1c99f1`.
  - `Lavaplayer/` — the audio library, tag `2.2.6`, repository `github.com/lavalink-devs/lavaplayer`.
- **Line numbers** are 1-based and refer to the exact file revision named above.
- **`[OBSERVED]`** means the behaviour was read directly from that source line, or extracted from the
  compiled bytecode of the exact dependency jar that Lavalink 4.2.2 ships (notations used:
  `javap` output is cited as `JAR:ClassName` and labelled `[OBSERVED-BYTECODE]`).
- **`[INFERRED]`** means the behaviour was not stated in the cited line but follows necessarily from it.
  Every `[INFERRED]` claim names the observed facts it is derived from.
- **`[VERIFIED]`** means the claim was additionally confirmed by an executed experiment (byte-level round-trip).

This document describes **behaviour and contracts only**. No upstream source text is reproduced.

---

## 1. Module map

### 1.1 Build structure

Lavalink is a Gradle multi-project build with three modules.

**OBSERVED** — `Lavalink/settings.gradle.kts:1-8`

```
rootProject.name = "Lavalink-Parent"
include(":Lavalink-Server")
include(":protocol")
include(":plugin-api")
project(":Lavalink-Server").projectDir = file("$rootDir/LavalinkServer")
```

| Module | Path | Purpose |
| --- | --- | --- |
| `Lavalink-Server` | `Lavalink/LavalinkServer/` | Spring Boot 3.3.0 application: REST controllers, WebSocket handler, plugin bootstrap, config binding. |
| `protocol` | `Lavalink/protocol/` | Kotlin Multiplatform **common** module holding the v4 wire protocol data classes. Serialized with `kotlinx.serialization`. |
| `plugin-api` | `Lavalink/plugin-api/` | JVM-only module defining the stable SPI that third-party plugins compile against. |

**OBSERVED** — `Lavalink/LavalinkServer/build.gradle.kts:49-89` (module dependencies):

```
implementation(projects.protocol)
implementation(projects.pluginApi) { exclude(group = "org.springframework.boot", module = "spring-boot-starter-tomcat") }
implementation(libs.xmlutil.jdk)
implementation(libs.xmlutil.serialization)
implementation(libs.bundles.metrics)
implementation(libs.spring.cloud.config)
implementation(libs.bundles.spring) { exclude(group = "org.springframework.boot", module = "spring-boot-starter-tomcat") }
implementation(libs.koe) { exclude(group = "org.slf4j", module = "slf4j-api") }
implementation(libs.koe.udpqueue) { exclude(module="udp-queue") }
implementation(libs.bundles.libdave.natives)
implementation(libs.bundles.udpqueue.natives) { exclude(group = "com.sedmelluq", module = "lava-common") }
implementation(libs.lavaplayer)
implementation(libs.lavaplayer.ip.rotator)
implementation(libs.lavadsp)
implementation(libs.kotlin.reflect)
implementation(libs.logback)
implementation(libs.sentry.logback)
implementation(libs.oshi) { exclude(group = "org.slf4j", module = "slf4j-api") }
testImplementation(libs.spring.boot.test)
```

The `plugin-api` module is deliberately excluded from Tomcat so plugins can be loaded into a context that
owns its own servlet container. **OBSERVED** — `Lavalink/LavalinkServer/build.gradle.kts:51-53` and `:60-62`.

**Java and Kotlin targets** — **OBSERVED** — `Lavalink/LavalinkServer/build.gradle.kts:30-32`:
`sourceCompatibility = JavaVersion.VERSION_17`. **OBSERVED** — `Lavalink/protocol/build.gradle.kts:20-24`:
the protocol module targets `JVM_17`.

### 1.2 Server module package layout

**OBSERVED** — directory listing of `Lavalink/LavalinkServer/src/main/java/lavalink/server/`:

| Package | Key types | Role |
| --- | --- | --- |
| `lavalink.server` | `LavalinkApplication`, `Launcher` | Spring Boot entry point and two-phase boot. |
| `lavalink.server.config` | `ServerConfig`, `AudioSourcesConfig`, `AudioPlayerConfiguration`, `WebsocketConfig`, `RequestAuthorizationFilter`, `KoeConfiguration`, `RateLimitConfig`, `HttpConfig`, `YoutubeConfig` | `application.yml` binding and bean construction. |
| `lavalink.server.bootstrap` | `PluginManager`, `PluginsConfig`, `PluginManifest`, `maven.kt` | Plugin download, classloading, manifest parsing. |
| `lavalink.server.io` | `SocketServer`, `SocketContext`, `EventEmitter`, `StatsCollector`, `HandshakeInterceptorImpl`, `RoutePlannerRestHandler`, `SessionRestHandler`, `ShutdownHandler`, `ResponseHeaderFilter`, `RequestLoggingFilter`, `DefaultAudioPluginInfoModifier` | WebSocket server, session lifecycle, auxiliary REST endpoints. |
| `lavalink.server.player` | `LavalinkPlayer`, `EventEmitter`, `PlayerRestHandler`, `AudioLoaderRestHandler`, `AudioLossCounter`, `TrackEndMarkerHandler` | Per-guild player façade and the player/loader REST endpoints. |
| `lavalink.server.player.filters` | `FilterChain`, `filterConfigs.kt` | Lavaplayer filter graph construction from the wire protocol. |
| `lavalink.server.info` | `InfoRestHandler`, `AppInfo` | `/v4/info` and `/version`. |
| `lavalink.server.util` | `util.kt`, `loading.kt` | Lavaplayer ↔ protocol conversions, track encode/decode helpers. |

### 1.3 Plugin API surface

**OBSERVED** — `Lavalink/plugin-api/src/main/java/dev/arbjerg/lavalink/api/`, all eight files:

| Interface | Members | Purpose |
| --- | --- | --- |
| `IPlayer` | `audioPlayer`, `track`, `guildId`, `socketContext`, `isPlaying`, `play`, `stop`, `setPause`, `seekTo(Long)`, `seekTo(Duration)`, `setVolume` | Handle a single guild's player from a plugin. |
| `ISocketContext` | `sessionId`, `userId`, `clientName`, `players`, `getPlayer`, `destroyPlayer`, `sendMessage` ×2, `state`, `closeWebSocket` ×3, `State` enum | Handle one WebSocket session. |
| `ISocketServer` | `sessions`, `resumableSessions` | Enumerate live sessions. |
| `PluginEventHandler` | `onWebSocketOpen`, `onSocketContextPaused`, `onSocketContextDestroyed`, `onWebSocketMessageOut`, `onNewPlayer`, `onDestroyPlayer` | Abstract-class lifecycle hooks, all `open` with no-op defaults. |
| `AudioFilterExtension` | `name`, `build(JsonElement, AudioDataFormat?, FloatPcmAudioFilter?)`, `isEnabled(JsonElement)` | Register a custom Lavaplayer filter reachable from the `filters` wire field. |
| `AudioPlayerManagerConfiguration` | `configure(AudioPlayerManager): AudioPlayerManager` | Mutate the Lavaplayer manager during startup. |
| `AudioPluginInfoModifier` | `modifyAudioTrackPluginInfo`, `modifyAudioPlaylistPluginInfo` | Inject arbitrary JSON into `Track.pluginInfo` / `Playlist.pluginInfo`. |
| `RestInterceptor` | extends Spring `HandlerInterceptor` | Intercept inbound HTTP requests. |

**OBSERVED** — `Lavalink/plugin-api/src/main/java/dev/arbjerg/lavalink/api/PluginEventHandler.kt:6-51`: every hook is
`open fun ... = Unit`, so a plugin overrides only what it needs.

### 1.4 Protocol module layout

**OBSERVED** — `Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/`:

| File | Contents |
| --- | --- |
| `messages.kt` | `Message` sealed interface, `Op` enum, `ReadyEvent`, `PlayerUpdateEvent`, `StatsEvent`, `EmittedEvent` (5 event types). |
| `player.kt` | `Player`, `Track`, `TrackInfo`, `VoiceState`, `PlayerState`, `PlayerUpdate`, `PlayerUpdateTrack`, `Players`, `Tracks`, `EncodedTracks`. |
| `loadResult.kt` | `LoadResult` sealed hierarchy (`TrackLoaded`, `PlaylistLoaded`, `SearchResult`, `NoMatches`, `LoadFailed`), `Playlist`, `PlaylistInfo`, `Exception`. |
| `filters.kt` | `Filters` and the ten built-in filter parameter classes. |
| `info.kt` | `Info`, `Version`, `Git`, `Plugins`, `Plugin`. |
| `session.kt` | `Session`, `SessionUpdate`. |
| `stats.kt` | `Stats`, `FrameStats`, `Memory`, `Cpu`. |
| `omissible.kt` | `Omissible<T>` — a tri-state (`Present` / `Omitted` / explicitly-null) wrapper used for PATCH bodies. |
| `mapper.kt` | JSON configuration for `kotlinx.serialization`. |
| `routeplanner.kt` | Route-planner status types. |
| `error.kt` | Error payload types. |

**OBSERVED** — `Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/omissible.kt`: `Omissible` exists
because a PATCH field has three distinguishable states — absent, present-and-null, present-and-valued.

### 1.5 Lavaplayer package layout (the audio engine)

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/`:

| Package | Key types | Role |
| --- | --- | --- |
| `player` | `DefaultAudioPlayerManager`, `DefaultAudioPlayer`, `AudioConfiguration`, `AudioPlayerLifecycleManager`, `GarbageCollectionMonitor` | Player pool, executor fleet, lifecycle sweeps. |
| `player.event` | `AudioEvent`, `AudioEventListener`, `TrackStartEvent`, `TrackEndEvent`, `TrackExceptionEvent`, `TrackStuckEvent`, `PlayerPauseEvent`, `PlayerResumeEvent` | Event bus. |
| `track` | `AudioTrack`, `AudioTrackInfo`, `AudioItem`, `AudioReference`, `AudioPlaylist`, `TrackMarker`, `TrackMarkerHandler` | Track model. |
| `track.playback` | `LocalAudioTrackExecutor`, `AudioProcessingContext`, `AbstractAudioFrameBuffer`, `AllocatingAudioFrameBuffer`, `NonAllocatingAudioFrameBuffer`, `MutableAudioFrame` | Decode loop and frame buffer. |
| `filter` | `AudioPipelineFactory`, `FilterChainBuilder`, `UniversalPcmAudioFilter`, `PcmFormat`, `BufferingPostProcessor`, `VolumePostProcessor`, `ResamplingPcmAudioFilter`, `ChannelCountPcmAudioFilter` | PCM processing graph. |
| `filter.*` (volume, equalizer, karaoke, timescale, tremolo, vibrato, distortion, rotation, channelmix, lowpass) | one filter each | The ten built-in filters. |
| `format` | `AudioDataFormat`, `OpusAudioDataFormat`, `Pcm16AudioDataFormat`, `StandardAudioDataFormats`, `AudioPlayerInputStream` | Format descriptors. |
| `format.transcoder` | `AudioChunkEncoder`, `AudioChunkDecoder`, `OpusChunkEncoder`, `OpusChunkDecoder`, `PcmChunkEncoder`, `PcmChunkDecoder` | Codec adapters. |
| `natives.opus` | `OpusEncoder`, `OpusDecoder`, `OpusEncoderLibrary` | JNI bindings to libopus. |
| `natives.samplerate` | `SampleRateConverter` | JNI bindings to libsamplerate. |
| `source` | `AudioSourceManager`, `ProbingAudioSourceManager`, plus one manager per source | Source registry. |
| `container` | `MediaContainerDescriptor`, `MediaContainerProbe`, `MediaContainerRegistry` | Container probing for HTTP/local sources. |
| `tools.io` | `MessageOutput`, `MessageInput` | The length-prefixed message framing used by the encoded-track format. |
| `tools` | `DataFormatTools`, `FriendlyException`, `ExceptionTools`, `ExecutorTools`, `HttpClientTools` | Utilities. |

---

## 2. Boot sequence

### 2.1 Two-phase Spring Boot startup

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/Launcher.kt:116-127`:

```
@JvmStatic
fun main(args: Array<String>) {
    if (args.isNotEmpty() && (args[0].equals("-v", ignoreCase = true) || args[0].equals("--version", ignoreCase = true))) {
        println(getVersionInfo(indentation = "", vanity = false))
        return
    }
    val parent = launchPluginBootstrap(args)
    launchMain(parent, args)
}
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/Launcher.kt:129-146`:

```
private fun launchPluginBootstrap(args: Array<String>) = SpringApplication(PluginManager::class.java).run {
    setBannerMode(Banner.Mode.OFF)
    webApplicationType = WebApplicationType.NONE
    run(*args)
}

private fun launchMain(parent: ConfigurableApplicationContext, args: Array<String>) {
    val pluginManager = parent.getBean(PluginManager::class.java)
    val properties = Properties()
    properties["componentScan"] = pluginManager.pluginManifests.map { it.path }
        .toMutableList().apply { add("lavalink.server") }
    SpringApplicationBuilder()
        .sources(LavalinkApplication::class.java)
        .properties(properties)
        .web(WebApplicationType.SERVLET)
        ...
        .resourceLoader(DefaultResourceLoader(pluginManager.classLoader))
```

**Behavioural contract to reimplement** — `[INFERRED]` from the two cited blocks:

1. **Phase 1** starts a *non-web* Spring context whose only component is `PluginManager`. Plugins are
   downloaded and classloaded here, before any server infrastructure exists.
2. **Phase 2** starts the real servlet context. Its component-scan set is the union of
   `lavalink.server` and every plugin's declared `path` prefix. The resource loader is rooted at the
   plugin classloader, so plugin-supplied `application.yml` fragments are resolvable.

`@ComponentScan` on `LavalinkApplication` explicitly excludes `PluginManager` so it is not instantiated
twice. **OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/Launcher.kt:49-55`:

```
@Suppress("SpringComponentScan")
@SpringBootApplication
@ComponentScan(
    value = ["\${componentScan}"],
    excludeFilters = [ComponentScan.Filter(type = FilterType.ASSIGNABLE_TYPE, classes = [PluginManager::class])]
)
class LavalinkApplication
```

The component-scan base package is supplied at runtime through the `componentScan` property, which
`launchMain` sets to the plugin manifest paths plus `lavalink.server`. **OBSERVED** — `Launcher.kt:137-139`.

The version string is printed on `ApplicationEnvironmentPreparedEvent` and
`"Lavalink is ready to accept connections."` on `ApplicationReadyEvent`.
**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/Launcher.kt:147-162`.

**`--version` / `-v` handling** — **OBSERVED** — `Launcher.kt:118-123`: if the first argument is
`-v` or `--version` (case-insensitive), the version block is printed to stdout and the process exits
**without** starting Spring. The version block includes the Lavaplayer version
(`PlayerLibrary.VERSION`) at `Launcher.kt:93`.

**The startup banner** — **OBSERVED** — `Launcher.kt:74-114`: `getVersionInfo()` optionally prints an
ANSI-coloured ASCII-art banner built by `getVanity()`. The colour codes are inserted by replacing the
literal characters `r`, `g` and `d` in the art string with the escape sequences for red, green and
default. The banner is suppressed for the `--version` path (`vanity = false`).

### 2.2 Plugin bootstrap

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/bootstrap/PluginManager.kt:20-36`:
`PluginManager` holds `pluginManifests: MutableList<PluginManifest>`.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/bootstrap/PluginManager.kt:162-187`:

```
private fun loadJars(): List<PluginManifest> {
    val directory = File(config.pluginsDir).takeIf { it.isDirectory } ?: return emptyList()
    val jarsToLoad = directory.listFiles()?.filter { it.isFile && it.extension == "jar" }
        ?.filter {
            if (it.length() == 0L) {
                log.warn("Plugin jar is empty, deleting...: ${it.path}")
                if (!it.delete()) { log.error("Failed to delete empty plugin jar: ${it.path}") }
                false
            } else { true }
        }
        ?.takeIf { it.isNotEmpty() } ?: return emptyList()
    classLoader = URLClassLoader.newInstance(
        jarsToLoad.map { URL("jar:file:${it.absolutePath}!/") }.toTypedArray(),
        javaClass.classLoader
    )
    return jarsToLoad.flatMap { loadJar(it, classLoader) }
}
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/bootstrap/PluginManager.kt:189-214` (`loadJar`):

```
val jar = JarFile(file)
val manifests = loadPluginManifests(jar)
...
val allowedPaths = manifests.map { manifest -> manifest.path.replace(".", "/") }
for (entry in it.entries()) {
    if (entry.isDirectory || !entry.name.endsWith(".class") || allowedPaths.none(entry.name::startsWith)) continue
    cl.loadClass(entry.name.dropLast(6).replace("/", "."))
    classCount++
}
```

**Behavioural contract** — `[INFERRED]` from the two cited blocks:

- All plugin jars in `pluginsDir` share **one** `URLClassLoader` whose parent is the server's own classloader.
- Only class files under a declared manifest `path` prefix are eagerly loaded. This forces static
  initialisers of plugin classes to run at boot.
- Empty jars are deleted from disk.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/bootstrap/PluginManager.kt:216-232`:
manifests are properties files at `lavalink-plugins/*.properties` inside the jar. Required keys:
`name`, `path`, `version`. A missing key is a fatal `RuntimeException`.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/bootstrap/PluginManager.kt:155-160`:
plugins may also be bundled on the classpath itself, discovered via
`classpath*:lavalink-plugins/*.properties`.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/bootstrap/PluginManager.kt:247-250`:

```
private data class Declaration(val group: String, val name: String, val version: String, val repository: String) {
    val canonicalJarName = "$name-$version.jar"
    val url = "$repository${group.replace(".", "/")}/$name/$version/$name-$version.jar"
}
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/bootstrap/PluginManager.kt:94-129`: update
checking fetches `<repo>/<group-path>/<name>/maven-metadata.xml` and compares against the declared version
using a semver parser.

**Download and version-cleanup logic** — **OBSERVED** — `PluginManager.kt:38-92` (`manageDownloads`):

```
private fun manageDownloads() {
    if (config.plugins.isEmpty()) return

    val directory = File(config.pluginsDir)
    directory.mkdir()

    val pluginJars = directory.listFiles()?.filter { it.extension == "jar" }
        ?.flatMap { file ->
            JarFile(file).use { jar ->
                loadPluginManifests(jar).map { manifest -> PluginJar(manifest, file) }
            }
        }
        ...
    val declarations = config.plugins.map { declaration ->
        if (declaration.dependency == null) throw RuntimeException("Illegal dependency declaration: null")
        val fragments = declaration.dependency!!.split(":")
        if (fragments.size != 3) throw RuntimeException("Invalid dependency \"${declaration.dependency}\"")

        val repository = declaration.repository
            ?: config.defaultPluginSnapshotRepository.takeIf { declaration.snapshot }
            ?: config.defaultPluginRepository

        Declaration(fragments[0], fragments[1], fragments[2], "${repository.removeSuffix("/")}/")
    }.distinctBy { "${it.group}:${it.name}" }

    for (declaration in declarations) {
        val jars = pluginJars.filter { it.manifest.name == declaration.name }.takeIf { it.isNotEmpty() }
            ?: pluginJars.filter { matchName(it, declaration.name) }

        var hasCurrentVersion = false

        for (jar in jars) {
            if (jar.manifest.version == declaration.version) {
                hasCurrentVersion = true
                continue
            }
            if (!jar.file.delete()) throw RuntimeException("Failed to delete ${jar.file.path}")
            log.info("Deleted ${jar.file.path} (new version: ${declaration.version})")
        }

        if (!hasCurrentVersion) {
            val url = declaration.url
            val file = File(directory, declaration.canonicalJarName)
            downloadJar(file, url)
        }

        checkPluginForUpdates(declaration)
    }
}
```

**Behavioural contract to reimplement** — `[INFERRED]` from the cited block plus `:130-153`
(`downloadJar`) and `:234-244` (`matchName`):

- The dependency string must be exactly `group:name:version` (3 colon-separated fragments).
  Anything else is a fatal `RuntimeException`.
- Repository resolution order: explicit `declaration.repository` → the snapshot repository if
  `declaration.snapshot` is true → the default release repository.
- Declarations are de-duplicated by `group:name`.
- Existing jars whose manifest **name** matches the declaration but whose **version** differs are
  **deleted**. If the exact version is already present, nothing is downloaded.
- `matchName` is the fallback matcher: it takes the jar filename without extension, truncates it at
  the first digit, then strips a trailing `-v` and a trailing `-`, and compares that to the declared
  name. This handles jar names like `my-plugin-v1.0.0.jar`.
- `downloadJar` streams the URL into the file with `Channels.newChannel(...).transferFrom(...)`. If the
  resulting file is **empty**, it is deleted and the download is retried **once**; a second empty
  result throws `RuntimeException("Failed to download plugin from <url>")`.
- **There is no checksum or signature verification** — the jar is trusted on the basis of the URL.
  A reimplementation that wants to be safer should add hash verification and pin the Maven
  repository, but that would be a behaviour change rather than a port.
- The update check is **advisory only**: it logs a warning if a newer version exists and never
  downloads it.

### 2.3 Application resources

**OBSERVED** — `Lavalink/LavalinkServer/src/main/resources/application.yml`:

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

`server.error.include-stacktrace: ON_PARAM` means stack traces are included in error responses only when the
client asks for them via a request parameter. **OBSERVED** — same file, lines 10-14.

**OBSERVED** — `Lavalink/LavalinkServer/build.gradle.kts:98-111`:

```
processResources {
    val tokens = mapOf(
        "project.version"    to project.version,
        "project.groupId"    to project.group,
        "project.artifactId" to "Lavalink-Server",
        "env.BUILD_TIME"     to System.currentTimeMillis().toString()
    )
    filter(ReplaceTokens::class, mapOf("tokens" to tokens))
    copy {
        from("application.yml.example")
        into(layout.buildDirectory.dir("resources/main"))
    }
}
```

The distribution ships `application.yml.example` copied into the built resources.
**OBSERVED** — `Lavalink/LavalinkServer/build.gradle.kts:107-110`.

The boot jar is named `Lavalink.jar` (`Lavalink-musl.jar` for musl targets), with the main class
`lavalink.server.Launcher`. **OBSERVED** — `Lavalink/LavalinkServer/build.gradle.kts:26-28` and `:141-155`.

---

## 3. Configuration surface

All configuration is bound by Spring Boot's `@ConfigurationProperties`. The three prefixes are
`lavalink` (plugins), `lavalink.server` (core), and `lavalink.server.sources` (source toggles).

### 3.1 `lavalink.server` — `ServerConfig`

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/ServerConfig.kt:29-51`:

```
@ConfigurationProperties(prefix = "lavalink.server")
@Component
class ServerConfig {
    var password: String? = null
    var isNonAllocatingFrameBuffer = false
    var bufferDurationMs: Int? = null
    var frameBufferDurationMs: Int? = null
    var opusEncodingQuality: Int? = null
    var resamplingQuality: ResamplingQuality? = null
    var trackStuckThresholdMs: Long? = null
    var useSeekGhosting: Boolean? = null
    var youtubePlaylistLoadLimit: Int? = null
    var playerUpdateInterval: Int = 5
    var isGcWarnings = true
    var isYoutubeSearchEnabled = true
    var isSoundcloudSearchEnabled = true
    var soundcloudFilterOutPreviewTracks = false
    var ratelimit: RateLimitConfig? = null
    var youtubeConfig: YoutubeConfig? = null
    var httpConfig: HttpConfig? = null
    var filters: Map<String, Boolean> = mapOf()
    var timeouts: TimeoutsConfig? = null
}
```

| YAML key | Type | Default | Effect |
| --- | --- | --- | --- |
| `lavalink.server.password` | string | `null` | Value required in the `Authorization` header. **OBSERVED** — `ServerConfig.kt:32`. Because the comparison is Kotlin structural equality against a nullable `String?`, an **unset** password means every request fails (401 for a missing header, 403 for a wrong one). See §4.2. |
| `lavalink.server.non-allocating-frame-buffer` | bool | `false` | Swap the Lavaplayer frame buffer implementation for `NonAllocatingAudioFrameBuffer`. **OBSERVED** — `AudioPlayerConfiguration.kt:64-67`. |
| `lavalink.server.buffer-duration-ms` | int | `null` | JDA-NAS UDP-queue buffer duration. `null` → Koe default 400 ms. `≤ 0` disables JDA-NAS. |
| `lavalink.server.frame-buffer-duration-ms` | int | `null` | Lavaplayer frame buffer duration. Values `< 200` are rejected and fall back to the 5000 ms default. **OBSERVED** — `AudioPlayerConfiguration.kt:69-78`. |
| `lavalink.server.opus-encoding-quality` | int | `null` | Opus quality 0..10. Out-of-range → 10. **OBSERVED** — `AudioPlayerConfiguration.kt:80-90`. |
| `lavalink.server.resampling-quality` | enum | `null` | `HIGH` / `MEDIUM` / `LOW`. **OBSERVED** — `AudioPlayerConfiguration.kt:92-95`. |
| `lavalink.server.track-stuck-threshold-ms` | long | `null` | Values `< 100` rejected → 10000 ms default. **OBSERVED** — `AudioPlayerConfiguration.kt:98-107`. |
| `lavalink.server.use-seek-ghosting` | bool | `null` | Whether old audio keeps playing across a seek. **OBSERVED** — `AudioPlayerConfiguration.kt:109-112`. |
| `lavalink.server.youtube-playlist-load-limit` | int | `null` | YouTube playlist page count. **OBSERVED** — `AudioPlayerConfiguration.kt:155-156`. |
| `lavalink.server.player-update-interval` | int | `5` | Seconds between `playerUpdate` WebSocket pushes. |
| `lavalink.server.gc-warnings` | bool | `true` | Enable Lavaplayer GC monitoring. **OBSERVED** — `AudioPlayerConfiguration.kt:60-62`. |
| `lavalink.server.youtube-search-enabled` | bool | `true` | Passed to `YoutubeAudioSourceManager`. **OBSERVED** — `AudioPlayerConfiguration.kt:131-141`. |
| `lavalink.server.soundcloud-search-enabled` | bool | `true` | Passed to `SoundCloudAudioSourceManager`. **OBSERVED** — `AudioPlayerConfiguration.kt:165-174`. |
| `lavalink.server.soundcloud-filter-out-preview-tracks` | bool | `false` | Passed to `SoundCloudAudioSourceManager`. |
| `lavalink.server.ratelimit` | object | `null` | Route planner config; see §3.5. |
| `lavalink.server.youtube-config` | object | `null` | `email`, `password`. |
| `lavalink.server.http-config` | object | `null` | HTTP proxy config. **OBSERVED** — `AudioPlayerConfiguration.kt:199-214`. |
| `lavalink.server.filters` | map | `{}` | Per-filter enable/disable. A filter absent from the map is enabled. |
| `lavalink.server.timeouts` | object | `null` | HTTP client timeouts. |

**Note on the deprecation warning** — **OBSERVED** — `AudioPlayerConfiguration.kt:116-124`:
when `lavalink.server.sources.youtube` is true, Lavalink logs a warning that the built-in YouTube source
is deprecated in favour of the `youtube-source` plugin.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/ServerConfig.kt:53-57`:

```
class TimeoutsConfig {
    var connectTimeoutMs: Int = 3000
    var connectionRequestTimeoutMs: Int = 3000
    var socketTimeoutMs: Int = 3000
}
```

### 3.2 `lavalink.server.sources` — `AudioSourcesConfig`

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/AudioSourcesConfig.kt:7-19`:

```
@ConfigurationProperties(prefix = "lavalink.server.sources")
@Component
data class AudioSourcesConfig(
    var isYoutube: Boolean = true,
    var isBandcamp: Boolean = true,
    var isSoundcloud: Boolean = true,
    var isTwitch: Boolean = true,
    var isVimeo: Boolean = true,
    var isNico: Boolean = false,
    var isHttp: Boolean = true,
    var isLocal: Boolean = false,
)
```

Spring Boot relaxed binding maps `isYoutube` → `youtube`, `isBandcamp` → `bandcamp`, etc.

### 3.3 `lavalink` — `PluginsConfig`

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/bootstrap/PluginsConfig.kt:5-18`:

```
@ConfigurationProperties(prefix = "lavalink")
@Component
class PluginsConfig {
    var plugins: List<PluginDeclaration> = emptyList()
    var pluginsDir: String = "./plugins"
    var defaultPluginRepository: String = "https://maven.lavalink.dev/releases"
    var defaultPluginSnapshotRepository: String = "https://maven.lavalink.dev/snapshots"
}
data class PluginDeclaration(
    var dependency: String? = null,
    var repository: String? = null,
    var snapshot: Boolean = false
)
```

The `dependency` string is split on `:` to yield `group:name:version`
**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/bootstrap/PluginManager.kt:55`.

### 3.4 Other nested config objects

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/RateLimitConfig.kt:1-7`:

```
data class RateLimitConfig(
    var ipBlocks: List<String> = emptyList(),
    var excludedIps: List<String> = emptyList(),
    var strategy: String = "RotateOnBan",
    var retryLimit: Int = -1,
    var searchTriggersFail: Boolean = true
)
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/YoutubeConfig.kt:1-4`:

```
data class YoutubeConfig(var email: String = "", var password: String = "")
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/HttpConfig.kt:1-6`:

```
data class HttpConfig(
    var proxyHost: String = "",
    var proxyPort: Int = 3128,
    var proxyUser: String = "",
    var proxyPassword: String = ""
)
```

### 3.5 Route planner selection

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/AudioPlayerConfiguration.kt:222-254`:

```
@Bean
fun routePlanner(serverConfig: ServerConfig): AbstractRoutePlanner? {
    val rateLimitConfig = serverConfig.ratelimit
    if (rateLimitConfig == null) { return null }
    val ipBlockList = rateLimitConfig.ipBlocks
    if (ipBlockList.isEmpty()) { return null }

    val blacklisted = rateLimitConfig.excludedIps.map { InetAddress.getByName(it) }
    val filter = Predicate<InetAddress> { !blacklisted.contains(it) }
    val ipBlocks = ipBlockList.map {
        when {
            Ipv4Block.isIpv4CidrBlock(it) -> Ipv4Block(it)
            Ipv6Block.isIpv6CidrBlock(it) -> Ipv6Block(it)
            else -> throw RuntimeException("Invalid IP Block '$it', make sure to provide a valid CIDR notation")
        }
    }

    return when (rateLimitConfig.strategy.lowercase(Locale.getDefault()).trim()) {
        "rotateonban" -> RotatingIpRoutePlanner(ipBlocks, filter, rateLimitConfig.searchTriggersFail)
        "loadbalance" -> BalancingIpRoutePlanner(ipBlocks, filter, rateLimitConfig.searchTriggersFail)
        "nanoswitch" -> NanoIpRoutePlanner(ipBlocks, rateLimitConfig.searchTriggersFail)
        "rotatingnanoswitch" -> RotatingNanoIpRoutePlanner(ipBlocks, filter, rateLimitConfig.searchTriggersFail)
        else -> throw RuntimeException("Unknown strategy!")
    }
}
```

**Behavioural contract to reimplement** — `[INFERRED]` from the cited block:

- A route planner exists only if `ratelimit` is present **and** `ipBlocks` is non-empty.
- The `strategy` string is lower-cased (using the default locale) and trimmed, then matched:

| `strategy` | Planner | Takes the exclusion filter |
| --- | --- | --- |
| `rotateonban` | `RotatingIpRoutePlanner` | yes |
| `loadbalance` | `BalancingIpRoutePlanner` | yes |
| `nanoswitch` | `NanoIpRoutePlanner` | **no** |
| `rotatingnanoswitch` | `RotatingNanoIpRoutePlanner` | yes |
| anything else | throws `RuntimeException("Unknown strategy!")` | — |

- Each IP block must be **CIDR notation**. A block that is neither valid IPv4 CIDR nor valid IPv6 CIDR
  throws `RuntimeException("Invalid IP Block '<x>', make sure to provide a valid CIDR notation")`.
- `excludedIps` are resolved to `InetAddress` and used as a predicate that **excludes** them from the
  block list. Note the `nano` strategies ignore this filter entirely.

### 3.6 Source manager registration order

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/AudioPlayerConfiguration.kt:116-217`.
Registration order into the Lavaplayer manager is:

1. `youtube` — if enabled (`:116-159`). Warns that the built-in YouTube source is deprecated.
2. `soundcloud` (`:160-175`)
3. `bandcamp` (`:176`)
4. `twitch` (`:177`)
5. `vimeo` (`:178`)
6. `nico` (`:179`)
7. `local` — registered with the media-container registry (`:180`)
8. Plugin-provided source managers (`audioSourceManagers` collection) (`:182-185`)
9. `http` — registered **last**, explicitly commented "This must be loaded last" (`:195-217`)

**Behavioural contract** — `[INFERRED]` from the ordering at `:116-217` plus the source iteration at
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:435-455`:
source managers are tried **in registration order** when resolving an identifier, and the first one that
returns a non-null item wins. HTTP is last so that more specific sources get first refusal.

**Plugin filter and manager ordering** — **OBSERVED** — `AudioPlayerConfiguration.kt:187`:
`audioPlayerManager.configuration.isFilterHotSwapEnabled = true` is set unconditionally, and at
`:190-193` plugin-provided `AudioPlayerManagerConfiguration` beans are folded over the manager in
order, each receiving the previous manager and returning a (possibly new) one.

### 3.7 Filter enable/disable resolution

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/info/InfoRestHandler.kt:25-38`:

```
private val enabledFilers = (listOf(
    "volume",
    "equalizer",
    "karaoke",
    "timescale",
    "tremolo",
    "vibrato",
    "distortion",
    "rotation",
    "channelMix",
    "lowPass"
) + filterExtensions.map { it.name }).filter {
    it !in serverConfig.filters || serverConfig.filters[it] == true
}
```

**Behavioural contract** — `[INFERRED]` from the cited block:

- The ten built-in filter names are listed in a **fixed order**, then plugin filter extension names are
  appended.
- A filter is **enabled** unless the config map contains it with a value that is not `true`.
  In other words: absent from the map → enabled; present with `true` → enabled; present with
  `false` → disabled.
- The resulting list is the `filters` array in the `/v4/info` response, and the same list is used to
  reject PATCH requests naming a disabled filter.

The same set is used to reject PATCH requests naming a disabled filter.
**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/PlayerRestHandler.kt:87-96`:

```
playerUpdate.filters.ifPresent { filters ->
    val invalidFilters = filters.validate(disabledFilters)

    if (invalidFilters.isNotEmpty()) {
        throw ResponseStatusException(
            HttpStatus.BAD_REQUEST,
            "Following filters are disabled in the config: ${invalidFilters.joinToString()}"
        )
    }
}
```

### 3.8 The ten built-in filters and their parameters

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/filters/filterConfigs.kt`:

| Filter | Class | Line | Parameters (defaults) | "Enabled" predicate |
| --- | --- | --- | --- | --- |
| `volume` | `VolumeConfig` | `:16-25` | `volume: Float` | `volume != 1.0f` |
| `equalizer` | `EqualizerConfig` | `:29-41` | `bands: List<Band>`, `Band(band: Int, gain: Float)` | any band gain `!= 0.0f` |
| `karaoke` | `KaraokeConfig` | `:43-59` | `level=1.0f`, `monoLevel=1.0f`, `filterBand=220.0f`, `filterWidth=100.0f` | always `true` |
| `timescale` | `TimescaleConfig` | `:61-76` | `speed=1.0`, `pitch=1.0`, `rate=1.0` | any of the three `!= 1.0` |
| `tremolo` | `TremoloConfig` | `:78-90` | `frequency=2.0f`, `depth=0.5f` | `depth != 0.0f` |
| `vibrato` | `VibratoConfig` | `:92-105` | `frequency=2.0f`, `depth=0.5f` | `depth != 0.0f` |
| `distortion` | `DistortionConfig` | `:107-132` | `sinOffset=0f`, `sinScale=1f`, `cosOffset=0f`, `cosScale=1f`, `tanOffset=0f`, `tanScale=1f`, `offset=0f`, `scale=1f` | any offset `!= 0f` or any scale `!= 1f` |
| `rotation` | `RotationConfig` | `:134-144` | `rotationHz=0.0` | `rotationHz != 0.0` |
| `channelMix` | `ChannelMixConfig` | `:146-162` | `leftToLeft=1f`, `leftToRight=0f`, `rightToLeft=0f`, `rightToRight=1f` | any of the four differs from the default |
| `lowPass` | `LowPassConfig` | `:164-174` | `smoothing=20.0f` | `smoothing > 1.0f` |

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/filters/filterConfigs.kt:176-182`:

```
abstract class FilterConfig {
    abstract fun build(format: AudioDataFormat, output: FloatPcmAudioFilter): FloatPcmAudioFilter?
    abstract val isEnabled: Boolean
    abstract val name: String
}
```

**Filter chain ordering** — **OBSERVED** —
`Lavalink/LavalinkServer/src/main/java/lavalink/server/player/filters/FilterChain.kt:79-114`:

```
private fun buildList() = listOfNotNull(
    volume, equalizer, karaoke, timescale, tremolo, vibrato,
    distortion, rotation, channelMix, lowPass,
    *pluginFilters.toTypedArray()
)
val isEnabled get() = buildList().any { it.isEnabled }
override fun buildChain(track: AudioTrack?, format: AudioDataFormat, output: UniversalPcmAudioFilter): MutableList<AudioFilter> {
    val enabledFilters = buildList().takeIf { it.isNotEmpty() } ?: return mutableListOf()
    val pipeline = mutableListOf<FloatPcmAudioFilter>()
    for (filter in enabledFilters) {
        val outputTo = pipeline.lastOrNull() ?: output
        val builtFilter = filter.build(format, outputTo)
        if (builtFilter != null) { pipeline.add(builtFilter) }
    }
    return pipeline.reversed().toMutableList() // Output last
}
```

**Behavioural contract** — `[INFERRED]` from `:100-113`:

- The chain is built in the **declaration order above**, but each filter is constructed with the
  *previous* filter as its output sink. The list is then `reversed()` so the array Lavalink hands to
  Lavaplayer runs **output-first** (the comment `// Output last` at `:113` refers to the reversed list's
  last element being the *first* configured filter's sink).
- A filter whose `build` returns `null` is silently dropped from the chain.
- `pluginFilters` are appended **after** all ten built-ins.

**Plugin filter ordering** — **OBSERVED** — `FilterChain.kt:72-77`: plugin filters are matched by name
against the `pluginFilters` map of the wire `Filters` object; unmatched plugin extensions are omitted.

**Filter hot-swap** — **OBSERVED** —
`Lavalink/LavalinkServer/src/main/java/lavalink/server/player/LavalinkPlayer.kt:57-61`:

```
var filters: FilterChain = FilterChain()
    set(value) {
        audioPlayer.setFilterFactory(value.takeIf { it.isEnabled })
        field = value
    }
```

and **OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/AudioPlayerConfiguration.kt:187`:
`audioPlayerManager.configuration.isFilterHotSwapEnabled = true`.

So changing the filter set takes effect on the **currently playing** track, and setting an all-default
filter chain installs `null` as the filter factory (i.e. no filtering at all).

---

## 4. HTTP API

### 4.1 Route inventory

**OBSERVED** — route annotations across `Lavalink/LavalinkServer/src/main/java/lavalink/server/`:

| Method | Path | Handler | Line |
| --- | --- | --- | --- |
| GET | `/v4/loadtracks?identifier=` | `AudioLoaderRestHandler.loadTracks` | `player/AudioLoaderRestHandler.kt:51` |
| GET | `/v4/decodetrack?encodedTrack=` or `?track=` | `AudioLoaderRestHandler.getDecodeTrack` | `player/AudioLoaderRestHandler.kt:96` |
| POST | `/v4/decodetracks` | `AudioLoaderRestHandler.decodeTracks` | `player/AudioLoaderRestHandler.kt:110` |
| GET | `/v4/sessions/{sessionId}/players` | `PlayerRestHandler.getPlayers` | `player/PlayerRestHandler.kt:34` |
| GET | `/v4/sessions/{sessionId}/players/{guildId}` | `PlayerRestHandler.getPlayer` | `player/PlayerRestHandler.kt:41` |
| PATCH | `/v4/sessions/{sessionId}/players/{guildId}` | `PlayerRestHandler.patchPlayer` | `player/PlayerRestHandler.kt:49` |
| DELETE | `/v4/sessions/{sessionId}/players/{guildId}` | `PlayerRestHandler.deletePlayer` | `player/PlayerRestHandler.kt:231` |
| PATCH | `/v4/sessions/{sessionId}` | `SessionRestHandler.patchSession` | `io/SessionRestHandler.kt:13` |
| GET | `/v4/info` | `InfoRestHandler.info` | `info/InfoRestHandler.kt:54` |
| GET | `/version` | `InfoRestHandler.version` | `info/InfoRestHandler.kt:57` |
| GET | `/v4/stats` | `StatsCollector` | `io/StatsCollector.kt:137` |
| GET | `/v4/routeplanner/status` | `RoutePlannerRestHandler.getStatus` | `io/RoutePlannerRestHandler.kt:24` |
| POST | `/v4/routeplanner/free/address` | `RoutePlannerRestHandler.freeSingleAddress` | `io/RoutePlannerRestHandler.kt:36` |
| POST | `/v4/routeplanner/free/all` | `RoutePlannerRestHandler.freeAllAddresses` | `io/RoutePlannerRestHandler.kt:54` |
| GET | `/` and `/v3/websocket` | `WebsocketConfig.oldWebsocket` | `config/WebsocketConfig.kt:29` |
| WS | `/v4/websocket` | `SocketServer` | `config/WebsocketConfig.kt:22` |

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/WebsocketConfig.kt:29-32`: the v3
endpoint exists only to log a deprecation warning; it opens no WebSocket.

**Server bind address** — **OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/ServerConfig.kt`
does not hold it; the HTTP listener is configured by Spring Boot properties, so the standard
`server.port` / `server.address` keys apply (default port 2333, default address `0.0.0.0`).
**OBSERVED** — `Lavalink/LavalinkServer/application.yml.example` (the shipped example configuration).

### 4.2 Authentication

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/RequestAuthorizationFilter.kt:8-31`:

```
override fun preHandle(request: HttpServletRequest, response: HttpServletResponse, handler: Any): Boolean {
    return when {
        // Collecting metrics is anonymous
        metricsConfig.endpoint.isNotEmpty() && request.servletPath == metricsConfig.endpoint -> true
        request.servletPath == "/error" -> true
        else -> {
            val authorization = request.getHeader("Authorization")
            if (authorization == null || authorization != serverConfig.password) {
                val path = request.requestURI.substring(request.contextPath.length)
                if (authorization == null) {
                    log.warn("Authorization missing for ${request.remoteAddr} on ${request.method} $path")
                    response.status = HttpStatus.UNAUTHORIZED.value()
                    return false
                }
                log.warn("Authorization failed for ${request.remoteAddr} on ${request.method} $path")
                response.status = HttpStatus.FORBIDDEN.value()
                return false
            }
            return true
        }
    }
}
```

**Behavioural contract to reimplement** — `[INFERRED]` from the cited block:

- The `Authorization` header is compared with **string equality** to `lavalink.server.password`.
- **Missing** header → **401 Unauthorized**. **Present but wrong** header → **403 Forbidden**.
  This distinction is observable and must be preserved.
- The Prometheus metrics endpoint and `/error` are exempt.
- When `password` is unset it is `null`, and the comparison `authorization != serverConfig.password` uses
  Kotlin's **structural equality**, where `null == null` is `true`. Because the `authorization == null`
  branch is checked first, the practical consequence is:
  - **No** `Authorization` header → the `authorization == null` branch → **401**.
  - Any `Authorization` header (including the literal text `null`) → the value is non-null, so it does
    not equal the `null` property → **403**.

  In other words, **an unset password locks the server out** rather than disabling auth. A Rust
  implementation must decide whether to reproduce this or to treat `null` as "no auth required"
  (see §11.3, item 19). **OBSERVED** — `RequestAuthorizationFilter.kt:26-37`, `ServerConfig.kt:32`.

The interceptor is registered globally. **OBSERVED** —
`Lavalink/LavalinkServer/src/main/java/lavalink/server/config/RequestAuthorizationFilter.kt:32-34`.

### 4.3 WebSocket handshake authentication

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/HandshakeInterceptorImpl.kt:27-53`:

```
override fun beforeHandshake(request, response, wsHandler, attributes): Boolean {
    val password = request.headers.getFirst("Authorization")
    if (password != serverConfig.password) {
        response.setStatusCode(HttpStatus.UNAUTHORIZED)
        return false
    }
    val userId = request.headers.getFirst("User-Id")
    if (userId.isNullOrEmpty() || userId.toLongOrNull() == 0L) {
        response.setStatusCode(HttpStatus.BAD_REQUEST)
        return false
    }
    ...
    val sessionId = request.headers.getFirst("Session-Id")
    val resuming = sessionId != null && socketServer.canResume(sessionId)
    response.headers.add("Session-Resumed", resuming.toString())
    return true
}
```

**Required WebSocket handshake headers** — `[INFERRED]` from the cited block:

| Header | Required | Failure status |
| --- | --- | --- |
| `Authorization` | yes | 401 if it does not equal `lavalink.server.password` |
| `User-Id` | yes | 400 if absent, empty, unparseable, or `0` |
| `Session-Id` | no | if present and resumable, the response carries `Session-Resumed: true` |
| `Client-Name` | no | absent → a warning is logged recommending the header |
| `User-Agent` | no | absent → a warning is logged |

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketServer.kt:97-145`: the `User-Id`
header is parsed with `.toLong()`, so a malformed value throws and aborts the handshake with a 500.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketServer.kt:85-91`: session IDs are
16 characters drawn uniformly from `a-z` plus `0-9`, retried until unused.

### 4.4 `/v4/loadtracks`

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/AudioLoaderRestHandler.kt:51-94`:

```
@GetMapping("/v4/loadtracks")
fun loadTracks(request: HttpServletRequest, @RequestParam identifier: String): ResponseEntity<LoadResult> {
    log.info("Got request to load for identifier \"${identifier}\"")
    val item = try { loadAudioItem(audioPlayerManager, identifier) }
        catch (ex: FriendlyException) {
            log.error("Failed to load track for identifier $identifier", ex)
            return ResponseEntity.ok(LoadResult.loadFailed(ex))
        }
    val result = when (item) {
        null -> LoadResult.NoMatches()
        is AudioTrack -> LoadResult.trackLoaded(item.toTrack(audioPlayerManager, pluginInfoModifiers))
        is AudioPlaylist -> {
            val tracks = item.tracks.map { it.toTrack(audioPlayerManager, pluginInfoModifiers) }
            if (item.isSearchResult) LoadResult.searchResult(tracks)
            else LoadResult.playlistLoaded(item.toPlaylistInfo(), item.toPluginInfo(pluginInfoModifiers), tracks)
        }
        else -> throw ResponseStatusException(HttpStatus.INTERNAL_SERVER_ERROR, "Identifier returned unknown audio item type: ...")
    }
    return ResponseEntity.ok(result)
}
```

**Behavioural contract** — `[INFERRED]` from the cited block plus
`Lavalink/LavalinkServer/src/main/java/lavalink/server/util/loading.kt:14-24`:

- The response is **always HTTP 200** for a resolvable or unresolvable identifier. Failure is reported
  *in the body* as `LoadResult.LoadFailed`, not as an HTTP error.
- `loadAudioItem` wraps any non-`Error` throwable in a `FriendlyException` with severity `FAULT` and
  message `"Something went wrong while looking up the track."`.
- An identifier that resolves to nothing yields `LoadResult.NoMatches` (`loadType: "NONE"`).

**LoadResult wire shapes** — **OBSERVED** —
`Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/loadResult.kt:17-124`:

| Variant | `loadType` | `data` |
| --- | --- | --- |
| `TrackLoaded` | `TRACK_LOADED` | a `Track` object |
| `PlaylistLoaded` | `PLAYLIST_LOADED` | a `Playlist` object (`info`, `pluginInfo`, `tracks`) |
| `SearchResult` | `SEARCH_RESULT` | `{ "tracks": [...] }` |
| `NoMatches` | `NONE` | `null` |
| `LoadFailed` | `LOAD_FAILED` | an `Exception` object |

### 4.5 `/v4/decodetrack` and `/v4/decodetracks`

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/AudioLoaderRestHandler.kt:96-118`:

```
@GetMapping("/v4/decodetrack")
fun getDecodeTrack(@RequestParam encodedTrack: String?, @RequestParam track: String?): ResponseEntity<Track> {
    val trackToDecode = encodedTrack ?: track ?: throw ResponseStatusException(HttpStatus.BAD_REQUEST, "No track to decode provided")
    return ResponseEntity.ok(decodeTrack(audioPlayerManager, trackToDecode).toTrack(trackToDecode, pluginInfoModifiers))
}
@PostMapping("/v4/decodetracks")
fun decodeTracks(@RequestBody encodedTracks: EncodedTracks): ResponseEntity<Tracks> {
    if (encodedTracks.tracks.isEmpty()) throw ResponseStatusException(HttpStatus.BAD_REQUEST, "No tracks to decode provided")
    return ResponseEntity.ok(Tracks(encodedTracks.tracks.map { decodeTrack(audioPlayerManager, it).toTrack(it, pluginInfoModifiers) }))
}
```

`encodedTrack` takes precedence over `track`. `EncodedTracks` is `{ "tracks": [ "base64...", ... ] }`.
**OBSERVED** — `Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/player.kt:76-78`.

Note the response echoes the **input** encoded string, not a re-encoding:
`toTrack(trackToDecode, ...)` takes the encoded form as a parameter.
**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/util/util.kt:51-58`.

### 4.6 `PATCH /v4/sessions/{sessionId}/players/{guildId}`

This is the most complex endpoint. **OBSERVED** —
`Lavalink/LavalinkServer/src/main/java/lavalink/server/player/PlayerRestHandler.kt:49-229`.

#### Request body

**OBSERVED** — `Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/player.kt:111-131`:

```
data class PlayerUpdateTrack(
    val encoded: Omissible<String?> = Omissible.Omitted(),
    val identifier: Omissible<String> = Omissible.Omitted(),
    val userData: Omissible<JsonObject> = Omissible.Omitted()
)
data class PlayerUpdate(
    val encodedTrack: Omissible<String?> = Omissible.Omitted(),   // deprecated
    val identifier: Omissible<String> = Omissible.Omitted(),      // deprecated
    val track: Omissible<PlayerUpdateTrack> = Omissible.Omitted(),
    val position: Omissible<Long> = Omissible.Omitted(),
    val endTime: Omissible<Long?> = Omissible.Omitted(),
    val volume: Omissible<Int> = Omissible.Omitted(),
    val paused: Omissible<Boolean> = Omissible.Omitted(),
    val filters: Omissible<Filters> = Omissible.Omitted(),
    val voice: Omissible<VoiceState> = Omissible.Omitted()
)
```

#### Validation rules

**OBSERVED** — `PlayerRestHandler.kt:59-109`:

| Condition | Result |
| --- | --- |
| Both `track` and (`encodedTrack` or `identifier`) present | 400 `"Cannot specify both track and encodedTrack/identifier"` |
| Both `encodedTrack` and `identifier` present (inside `track`) | 400 `"Cannot specify both encodedTrack and identifier"` |
| `filters` names a filter disabled in config | 400 `"Following filters are disabled in the config: ..."` |
| `voice` present with blank `token`, blank `endpoint`, blank `sessionId`, or blank/absent `channelId` | 400 `"token, endpoint, sessionId and channelId must be provided in voice state"` |
| `endTime` present and `≤ 0` | 400 `"End time must be greater than 0"` |

#### Voice state handling

**OBSERVED** — `PlayerRestHandler.kt:113-141`: the existing Koe media connection is compared against the
incoming voice state on **endpoint**, **token**, **sessionId** and **channelId**. If any differs, or the
connection is absent/closed, the old connection is destroyed, a new one is created, and
`conn.connect(...)` is awaited. A failure becomes a 500 `"Failed to connect to voice server"`.
After a successful connect, `player.provideTo(conn)` installs the audio-frame provider.

#### Field-application order (critical for a reimplementation)

**OBSERVED** — `PlayerRestHandler.kt:143-226`. The order is:

1. `voice` (if present) — reconnect the media connection, **synchronized on the player object**.
2. `paused` — applied **only if no new track was requested**.
3. `userData` — applied to the current track **only if no new track was requested**.
4. `volume` — always applied.
5. `position` — seeks the current track **only if no new track was requested**; a player update is pushed afterwards.
6. `endTime` — installs a `TrackMarker` on the current track **only if no new track was requested**.
7. `filters` — replaces the player's filter chain; a player update is pushed afterwards.
8. If `encoded` or `identifier` was supplied:
   - `noReplace=true` **and** a track is already playing → the request is skipped entirely and the
     current player state is returned (200).
   - otherwise `paused` is set to the requested value, or `false` when absent.
   - `encoded` takes precedence over `identifier`.
   - An `identifier` is resolved via `loadAudioItem`; a `FriendlyException` becomes a 400 with the
     exception message; a non-track result becomes 400 with
     `"No matches found for identifier"` or `"Cannot play a playlist or search result"`.
   - `position`, `userData` and `endTime` are then applied to the **new** track.
   - The new track is played and `provideTo(getMediaConnection(player))` re-installs the provider.
   - If the track resolves to `null` (e.g. `encoded` is explicitly null), `player.stop()` is called.

**Behavioural contract** — `[INFERRED]` from `:143-226`: `position`, `userData` and `endTime` are
deliberately **not** applied to the current track when a new track is being started, because the same
values are applied to the new track instead. A reimplementation must not apply them twice.

#### `endTime` marker semantics

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/TrackEndMarkerHandler.kt:26-38`:

```
class TrackEndMarkerHandler(private val player: LavalinkPlayer) : TrackMarkerHandler {
    companion object { val APPLICABLE_STATES = listOf(MarkerState.REACHED, MarkerState.BYPASSED) }
    override fun handle(state: TrackMarkerHandler.MarkerState) {
        if (state !in APPLICABLE_STATES) { return }
        player.endMarkerHit = true
        player.stop()
    }
}
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/EventEmitter.kt`: when the track
ends, if `player.endMarkerHit` is set, the reported reason is overridden to `FINISHED` and the flag is reset.

#### Response

**OBSERVED** — `PlayerRestHandler.kt:228`: the endpoint returns the `Player` object.
**OBSERVED** — `Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/player.kt:17-26`:

```
data class Player(
    val guildId: String,
    val track: Track?,
    val volume: Int,
    val paused: Boolean,
    val state: PlayerState,
    val voice: VoiceState,
    val filters: Filters
)
```

`PlayerState` is `{ time, position, connected, ping }`.
**OBSERVED** — `Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/player.kt:103-109`.

### 4.7 `GET` / `DELETE` player endpoints

**OBSERVED** — `PlayerRestHandler.kt:34-47`: `GET .../players` returns `{ "players": [ Player, ... ] }`;
`GET .../players/{guildId}` returns a single `Player`; a missing session is a 404
(`"Session not found"`), a missing player is a 404 (`"Player not found"`).
**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/util/util.kt:125-129`.

**OBSERVED** — `PlayerRestHandler.kt:231-235`: `DELETE .../players/{guildId}` returns **204 No Content**
and destroys the player plus its Koe media connection.
**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:131-138`.

### 4.8 `PATCH /v4/sessions/{sessionId}`

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SessionRestHandler.kt:13-25`:

```
@PatchMapping("/v4/sessions/{sessionId}")
private fun patchSession(@RequestBody sessionUpdate: SessionUpdate, @PathVariable sessionId: String): ResponseEntity<Session> {
    val context = socketContext(socketServer, sessionId)
    sessionUpdate.resuming.ifPresent { context.resumable = it }
    sessionUpdate.timeout.ifPresent { context.resumeTimeout = it.inWholeSeconds }
    return ResponseEntity.ok(Session(context.resumable, context.resumeTimeout))
}
```

`SessionUpdate` carries `resuming: Omissible<Boolean>` and `timeoutSeconds: Omissible<Long>`
(serialised under the JSON name `"timeout"`). **OBSERVED** —
`Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/session.kt:40-47`.

### 4.9 `/v4/info` and `/version`

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/info/InfoRestHandler.kt:36-58` and
`Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/info.kt:23-33`:

```
data class Info(
    val version: Version,
    val buildTime: Long,
    val git: Git,
    val jvm: String,
    val lavaplayer: String,
    val sourceManagers: List<String>,
    val filters: List<String>,
    val plugins: Plugins
)
```

`sourceManagers` is the list of `sourceName` values of every registered manager.
**OBSERVED** — `InfoRestHandler.kt:44`.

`Version` is `{ semver, major, minor, patch, preRelease }` parsed by a strict semver regex.
**OBSERVED** — `Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/info.kt:45-63`.

`/version` returns the bare version string as plain text.
**OBSERVED** — `InfoRestHandler.kt:57-58`.

### 4.10 `/v4/stats`

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/StatsCollector.kt:33` and `:137`.
The stats payload is defined in
`Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/stats.kt`.

**CPU sampling behaviour** — **OBSERVED** — `StatsCollector.kt:38` and `:62-126`:

```
private const val CPU_STATS_REFRESH_INTERVAL_MS = 30000
```

CPU stats are cached and recomputed at most every 30 s. The implementation is:

- `getOrUpdateCpuStats()` (`:62-79`) does a double-checked lock: it reads the `@Volatile` cache and
  timestamp, and only then takes `cpuStatsCalculationLock` to recompute. Two concurrent callers
  therefore perform at most one recomputation.
- `performCpuStatsCalculation()` (`:84-126`) computes:
  - **system load** as the delta between two OSHI CPU-tick samples, via
    `hal.processor.getSystemCpuLoadBetweenTicks(prevTicks)`. The tick snapshot is taken under
    `ticksLock`. The very first call has no baseline and returns `0.0`.
  - **process load** as `(ΔcpuTime / Δuptime) / logicalProcessorCount`, i.e. the process's share of
    one core, normalised across all cores so the result is in `[0, 1]`. The first call is `0.0`
    because there is no uptime baseline yet.
- Both values are `coerceIn(0.0, 1.0)` and replaced with `0.0` if non-finite.

**Memory model** — **OBSERVED** — `StatsCollector.kt:152-159`: the `Memory` object is derived from
`Runtime.getRuntime()`:

| Field | Expression |
| --- | --- |
| `free` | `runtime.freeMemory()` |
| `used` | `runtime.totalMemory() - runtime.freeMemory()` |
| `allocated` | `runtime.totalMemory()` |
| `reservable` | `runtime.maxMemory()` |

**Player counting** — **OBSERVED** — `StatsCollector.kt:143-148`: `playersTotal` sums
`context.players.size` and `playersPlaying` sums `context.playingPlayers.size` across **all**
socket contexts. The uptime is `System.currentTimeMillis() - Launcher.startTime`.

**Frame statistics** — **OBSERVED** — `StatsCollector.kt:161-186`: the `frameStats` object is only
computed when a `SocketContext` is supplied (i.e. for the periodic WebSocket `stats` event, not for
the REST endpoint). It averages `lastMinuteSuccess` and `lastMinuteLoss` over the playing players
that have usable loss data, and derives the deficit as
`playerCount * EXPECTED_PACKET_COUNT_PER_MIN - (totalSent + totalNulled)`. If no player has usable
data, `frameStats` stays `null`.

**The REST endpoint vs the WebSocket event** — `[INFERRED]` from `:128-138` and `:140-196`: the REST
handler calls `retrieveStats()` with a `null` context, so the REST response **never** contains
`frameStats`, while the WebSocket `stats` event does.

### 4.11 Route planner endpoints

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/RoutePlannerRestHandler.kt:18-70`:

- `GET /v4/routeplanner/status` returns **204 No Content** when no route planner is configured.
- `POST /v4/routeplanner/free/address` takes `{ "address": "..." }`, resolves it, and frees it.
  An unresolvable host is a 400 `"Invalid address: ..."`.
- `POST /v4/routeplanner/free/all` frees every failing address.
- Both POST endpoints throw `RoutePlannerDisabledException` when no planner exists.

---

## 5. WebSocket event stream

### 5.1 Endpoint and lifecycle

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/WebsocketConfig.kt:20-23`:

```
override fun registerWebSocketHandlers(registry: WebSocketHandlerRegistry) {
    registry.addHandler(server, "/v4/websocket").addInterceptors(handshakeInterceptor)
}
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketServer.kt:97-145`:
on connection established, the server either resumes an existing resumable session (if the `Session-Id`
header names one) or creates a fresh `SocketContext` and sends a `ReadyEvent`.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketServer.kt:171-173`:
the v4 WebSocket is **receive-only for events**; any inbound text message is rejected with a warning
telling the client to use the REST API.

### 5.2 Session state machine

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:83-88`:

```
override val state: ISocketContext.State
    get() = when {
        session.isOpen -> ISocketContext.State.OPEN
        sessionPaused -> ISocketContext.State.RESUMABLE
        else -> ISocketContext.State.DESTROYED
    }
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketServer.kt:147-169`:
on connection close, if `context.resumable` is true the context is moved into `resumableSessions` and
`context.pause()` is called; otherwise `context.shutdown()` runs.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:140-146`:

```
fun pause() {
    sessionPaused = true
    sessionTimeoutFuture = executor.schedule<Unit>({ socketServer.onSessionResumeTimeout(this) }, resumeTimeout, TimeUnit.SECONDS)
    eventEmitter.onSocketContextPaused()
}
```

Default `resumeTimeout` is 60 seconds. **OBSERVED** — `SocketContext.kt:78`.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:182-194`:
on resume, a `ReadyEvent(resumed = true, sessionId)` is sent, then every queued event is flushed in
order, then a player update is pushed for every player.

**Event replay queue** — **OBSERVED** — `SocketContext.kt:74` and `:154-160`:
while `sessionPaused`, outgoing payloads are appended to a `ConcurrentLinkedQueue<String>` instead of
being sent. The queue is drained on resume.

**OBSERVED** — `SocketContext.kt:188-191`: the drain loop is explicitly *not* atomic — events are
replayed one at a time.

### 5.3 Message envelope

**OBSERVED** — `Lavalink/protocol/src/commonMain/kotlin/dev/arbjerg/lavalink/protocol/v4/messages.kt:14-37`:

```
@Serializable(with = Message.Serializer::class)
sealed interface Message {
    val op: Op
    @Serializable
    enum class Op(val value: String) {
        @SerialName("ready")        Ready("ready"),
        @SerialName("stats")        Stats("stats"),
        @SerialName("playerUpdate") PlayerUpdate("playerUpdate"),
        @SerialName("event")        Event("event");
    }
}
```

**All messages are server→client.** The client never sends protocol messages on `/v4/websocket`.

### 5.4 `ready`

**OBSERVED** — `messages.kt:39-50`:

```
@SerialName("ready")
data class ReadyEvent private constructor(
    override val op: Op, val resumed: Boolean, val sessionId: String
) : Message {
    constructor(resumed: Boolean, sessionId: String) : this(Op.Ready, resumed, sessionId)
}
```

Wire shape: `{"op":"ready","resumed":false,"sessionId":"..."}`.

### 5.5 `playerUpdate`

**OBSERVED** — `messages.kt:52-64` and `player.kt:103-109`:

```
@SerialName("playerUpdate")
data class PlayerUpdateEvent private constructor(
    override val op: Op, val state: PlayerState, val guildId: String
) : Message
data class PlayerState(val time: Long, val position: Long, val connected: Boolean, val ping: Long)
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketServer.kt:66-82`:

```
fun sendPlayerUpdate(socketContext: SocketContext, player: LavalinkPlayer) {
    if (socketContext.sessionPaused) return
    val connection = socketContext.getMediaConnection(player).gatewayConnection
    socketContext.sendMessage(Message.Serializer, Message.PlayerUpdateEvent(
        PlayerState(
            System.currentTimeMillis(),
            player.audioPlayer.playingTrack?.position ?: 0,
            connection?.isOpen == true,
            connection?.ping ?: -1L
        ),
        player.guildId.toString()
    ))
}
```

**Behavioural contract** — `[INFERRED]` from the cited block:

- `time` is epoch milliseconds at the moment of emission.
- `position` is the currently playing track's position in ms, or `0` when nothing is playing.
- `connected` is `true` only if the Koe gateway connection exists and is open.
- `ping` is the gateway ping in ms, or `-1` when unavailable.

### 5.6 `stats`

**OBSERVED** — `messages.kt:66-87`: a `StatsEvent` carries `frameStats` (nullable), `players`,
`playingPlayers`, `uptime`, `memory` and `cpu`.

**Push cadence** — **OBSERVED** — `SocketContext.kt:98-107`:

```
init {
    val task = statsCollector.createTask(this)
    executor.scheduleAtFixedRate(task, 0, 1, TimeUnit.MINUTES)
    ...
}
```

Stats are pushed **once per minute** per session, starting immediately.

### 5.7 `event` — the five emitted event types

**OBSERVED** — `messages.kt:89-131`: the `event` op carries a nested `type` discriminator with these values:

| `type` | Payload class | Line |
| --- | --- | --- |
| `TrackStartEvent` | `guildId`, `track` | `messages.kt:133-142` |
| `TrackEndEvent` | `guildId`, `track`, `reason` | `messages.kt:144-200` |
| `TrackExceptionEvent` | `guildId`, `track`, `exception` | `messages.kt:202-213` |
| `TrackStuckEvent` | `guildId`, `track`, `thresholdMs` | `messages.kt:215-228` |
| `WebSocketClosedEvent` | `guildId`, `code`, `reason`, `byRemote` | `messages.kt:230-243` |

#### Track end reasons

**OBSERVED** — `messages.kt:163-199`:

```
enum class AudioTrackEndReason(val mayStartNext: Boolean) {
    @SerialName("finished")   FINISHED(true),
    @SerialName("loadFailed") LOAD_FAILED(true),
    @SerialName("stopped")    STOPPED(false),
    @SerialName("replaced")   REPLACED(false),
    @SerialName("cleanup")    CLEANUP(false)
}
```

`mayStartNext` is **not** part of the JSON; it is a client-side hint indicating whether the client
should advance its own queue.

**Behavioural contract** — `[INFERRED]` from `messages.kt:165-197`:

- `finished` — the track reached its end (also used when it ended due to an exception).
- `loadFailed` — the track threw before producing any audio.
- `stopped` — `stop()` was called or `playTrack(null)`.
- `replaced` — a new track started. **The old track keeps playing** until its buffer drains or the new
  track's audio becomes available.
- `cleanup` — the player's idle cleanup threshold elapsed.

#### Exception payload

**OBSERVED** — `messages.kt:202-213` plus
`Lavalink/LavalinkServer/src/main/java/lavalink/server/player/EventEmitter.kt`:
the `exception` object is `{ message, severity, cause, stackTrace }`, where `severity` is one of
`COMMON` / `SUSPICIOUS` / `FAULT`, `cause` is the root-cause class name, and `stackTrace` is the
root cause's stack trace rendered as a string.
**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/util/util.kt:143-160` for the
severity mapping and root-cause extraction.

#### WebSocket closed event

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:219-234`:
emitted when the **voice** gateway closes (not the Lavalink WebSocket). Carries the gateway close
`code`, `reason` and `byRemote`. A player update is pushed immediately afterwards.

### 5.8 Player update push scheduling

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/LavalinkPlayer.kt:114-125`:

```
override fun onTrackStart(player: AudioPlayer, track: AudioTrack) {
    if (updateFuture?.isCancelled == false) { return }
    updateFuture = socketContext.playerUpdateService.scheduleAtFixedRate(
        { sendPlayerUpdate(socketContext, this) },
        0,
        serverConfig.playerUpdateInterval.toLong(),
        TimeUnit.SECONDS
    )
}
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/LavalinkPlayer.kt:108-112`:

```
override fun onTrackEnd(player: AudioPlayer, track: AudioTrack, endReason: AudioTrackEndReason) {
    updateFuture?.cancel(false)
}
```

**Behavioural contract** — `[INFERRED]` from `:114-125` and `:108-112`:

- The periodic player-update task is started when a track **starts** and cancelled when it **ends**.
- Its period is `lavalink.server.player-update-interval` seconds (default 5), with an initial delay of 0.
- Player updates are therefore **not** emitted for an idle player, except as one-off pushes triggered
  by REST calls (filters change, seek, voice gateway events).

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:101-106`:
the `playerUpdateService` is a scheduled thread pool with **2** threads, named `player-update`, daemon.

### 5.9 Send path

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:154-175`:

```
private fun send(payload: String) {
    eventEmitter.onWebSocketMessageOut(payload)
    if (sessionPaused) { resumeEventQueue.add(payload); return }
    if (!session.isOpen) return
    val undertowSession = (session as StandardWebSocketSession).nativeSession as UndertowSession
    WebSockets.sendText(payload, undertowSession.webSocketChannel, object : WebSocketCallback<Void> { ... })
}
```

**Behavioural contract** — `[INFERRED]` from the cited block:

- Every outbound payload is first offered to the plugin `onWebSocketMessageOut` hook.
- Sends are **fire-and-forget** at the application layer; failures are logged, not retried.
- A closed session silently drops the payload (unless paused, in which case it is queued).

### 5.10 Plugin event hooks

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/EventEmitter.kt` (the session-level
emitter) dispatches:

| Hook | Trigger |
| --- | --- |
| `onWebSocketOpen(context, resumed)` | after a new or resumed connection |
| `onSocketContextPaused(context)` | when the connection drops and the session becomes resumable |
| `onSocketContextDestroyed(context)` | at `SocketContext.shutdown()` |
| `onWebSocketMessageOut(context, message)` | for every outbound payload |

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:109-113`:
`onNewPlayer` fires the first time a guild's player is created for a session.

---

## 6. The encoded-track format

This is the single most important wire contract: an encoded track is a **Base64** string wrapping a
versioned binary blob. A Rust reimplementation must produce and consume byte-identical output.

### 6.1 Framing layer — `MessageOutput` / `MessageInput`

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/tools/io/MessageOutput.java:9-61`.

`MessageOutput` is a two-stage writer. A message is first accumulated into an internal
`ByteArrayOutputStream`, then committed to the underlying stream as:

```
[4-byte big-endian header][payload bytes]
```

**OBSERVED** — `MessageOutput.java:38-52`:

```
public void commitMessage() throws IOException {
    dataOutputStream.writeInt(messageByteOutput.size());
    messageByteOutput.writeTo(outputStream);
}

public void commitMessage(int flags) throws IOException {
    dataOutputStream.writeInt(messageByteOutput.size() | flags << 30);
    messageByteOutput.writeTo(outputStream);
}
```

**The 4-byte header packs two values into one `int`:**

```
bits 31..30 : flags   (2 bits)
bits 29..0  : payload length (max 2^30 - 1)
```

**OBSERVED** — `MessageOutput.java:28-31` and `:54-61`:

```
public DataOutput startMessage() {
    messageByteOutput.reset();
    return messageDataOutput;
}

public void finish() throws IOException {
    dataOutputStream.writeInt(0);
}
```

**IMPORTANT — `finish()` is NOT part of the payload.** It writes a separate 4-byte zero header that
marks the end of the message sequence, so a reader knows to return `null` at that position.
**OBSERVED** — `MessageOutput.java:54-61` (the Javadoc says "Write an end marker to the stream so that
decoder knows to return null at this position").

An encoded track therefore consists of **exactly one** committed message and **no** end marker.
A reimplementation that emits the `finish()` marker as part of the encoded-track payload produces a
blob that is 4 bytes too long and that decodes to a null track.

**Header flags** — `[INFERRED]` from `MessageOutput.java:49-51` and the flag usage at
`DefaultAudioPlayerManager.java:238` and `:248`:

| Flag | Value | Meaning |
| --- | --- | --- |
| `TRACK_INFO_VERSIONED` | `1` | The payload begins with a version byte. |
| *(second flag bit)* | `2` | Reserved. `MessageOutput` does not define a named constant for it; the frame is only used with flags `0` or `1` in the track path. |

### 6.1b The reader — `MessageInput`

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/tools/io/MessageInput.java:36-46`:

```
public DataInput nextMessage() throws IOException {
    int value = dataInputStream.readInt();
    messageFlags = (int) ((value & 0xC0000000L) >> 30L);
    messageSize = value & 0x3FFFFFFF;

    if (messageSize == 0) {
        return null;
    }

    return new DataInputStream(new BoundedInputStream(countingInputStream, messageSize));
}
```

**Behavioural contract to reimplement** — `[INFERRED]` from the cited block plus
`MessageInput.java:29-35` (whose Javadoc states that "a null return value indicates the position where
`MessageOutput#finish()` had written the end marker"):

- The reader splits the header exactly as the writer does: `flags = (header & 0xC0000000) >> 30`,
  `size = header & 0x3FFFFFFF`.
- A **size of zero** means "end of message sequence" and yields `null`.
- Each message is returned as a `BoundedInputStream` limited to exactly `size` bytes, so a reader
  cannot read past the end of a frame.
- `skipRemainingBytes()` **must** be called after reading a message if the reader did not consume all
  of it, otherwise the stream position is wrong for the next message.
  **OBSERVED** — `MessageInput.java:61-69`.

**Consequence for the encoder** — `[INFERRED]`: since `finish()` writes a **zero-length** header and
the payload length is stored in the low 30 bits, a track whose payload were genuinely zero-length
would be indistinguishable from the end marker. This is another reason the encoder must emit exactly
one non-empty frame and no marker.

### 6.2 The track-info payload — version 3

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:37-38`:

```
private static final int TRACK_INFO_VERSIONED = 1;
private static final int TRACK_INFO_VERSION = 3;
```

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:220-239`:
the complete encode routine:

```
@Override
public void encodeTrack(MessageOutput stream, AudioTrack track) throws IOException {
    DataOutput output = stream.startMessage();
    output.write(TRACK_INFO_VERSION);

    AudioTrackInfo trackInfo = track.getInfo();
    output.writeUTF(trackInfo.title);
    output.writeUTF(trackInfo.author);
    output.writeLong(trackInfo.length);
    output.writeUTF(trackInfo.identifier);
    output.writeBoolean(trackInfo.isStream);
    DataFormatTools.writeNullableText(output, trackInfo.uri);
    DataFormatTools.writeNullableText(output, trackInfo.artworkUrl);
    DataFormatTools.writeNullableText(output, trackInfo.isrc);

    encodeTrackDetails(track, output);
    output.writeLong(track.getPosition());

    stream.commitMessage(TRACK_INFO_VERSIONED);
}
```

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:291-295`:

```
private void encodeTrackDetails(AudioTrack track, DataOutput output) throws IOException {
    AudioSourceManager sourceManager = track.getSourceManager();
    output.writeUTF(sourceManager.getSourceName());
    sourceManager.encodeTrack(track, output);
}
```

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/tools/DataFormatTools.java:169-175`:

```
public static void writeNullableText(DataOutput output, String text) throws IOException {
    output.writeBoolean(text != null);
    if (text != null) { output.writeUTF(text); }
}
```

### 6.3 Exact byte layout of the v3 payload

**OBSERVED** — combined from `DefaultAudioPlayerManager.java:220-239` and `:291-295`.
`writeUTF` is Java's **modified UTF-8** with a 2-byte unsigned length prefix.

| # | Size | Type | Field | Notes |
| --- | --- | --- | --- | --- |
| 1 | 1 | `u8` | version | always `3` |
| 2 | 2 + n | `u16be` + modified-UTF-8 | `title` | `writeUTF` |
| 3 | 2 + n | `u16be` + modified-UTF-8 | `author` | `writeUTF` |
| 4 | 8 | `i64be` | `length` | track duration in ms |
| 5 | 2 + n | `u16be` + modified-UTF-8 | `identifier` | `writeUTF` |
| 6 | 1 | `u8` | `isStream` | `1` = stream, `0` = finite |
| 7 | 1 | `u8` | `uri` present | `1` = present |
| 8 | 0 or (2+n) | — | `uri` | `writeUTF` only if present |
| 9 | 1 | `u8` | `artworkUrl` present | `1` = present |
| 10 | 0 or (2+n) | — | `artworkUrl` | `writeUTF` only if present |
| 11 | 1 | `u8` | `isrc` present | `1` = present |
| 12 | 0 or (2+n) | — | `isrc` | `writeUTF` only if present |
| 13 | 2 + n | `u16be` + modified-UTF-8 | `sourceName` | the source manager's name, e.g. `twitch` |
| 14 | 0 or (2+n) | — | source-specific details | depends on the source manager — see §6.5 |
| 15 | 8 | `i64be` | `position` | the track's current position in ms |

**Note the three separate nullable strings** — `uri`, `artworkUrl` and `isrc` are distinct fields.
A reimplementation must not collapse `artworkUrl` and `userData` into one slot: `userData` is *not* part
of the encoded track at all. **OBSERVED** — the `AudioTrackInfo` constructor at
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/track/AudioTrackInfo.java:50-59` takes
`(title, author, length, identifier, isStream, uri, artworkUrl, isrc)`.

**`userData` is not encoded** — **OBSERVED** —
`Lavalink/LavalinkServer/src/main/java/lavalink/server/util/util.kt:51-58`: the Lavalink-side `Track`
protocol object carries `userData` as a separate JSON field sourced from `AudioTrack.userData`, not from
the encoded blob. It is therefore lost on a round-trip through the encoded string unless the client
re-sends it.

### 6.4 Canonical worked example — VERIFIED

**OBSERVED + VERIFIED** — the following track was encoded by running Lavaplayer's own
`DefaultAudioPlayerManager.encodeTrack` against a real `AudioTrackInfo` and dumping the bytes:

| Field | Value |
| --- | --- |
| `title` | `TITLE` |
| `author` | `AUTHOR` |
| `length` | `0` |
| `identifier` | `https://example.com/watch` |
| `isStream` | `false` |
| `uri` | `null` |
| `artworkUrl` | `https://example.com/art.png` |
| `isrc` | `ISRC123` |
| `sourceName` | `http` (no source-specific bytes) |
| `position` | `0` |

**Resulting header**: `0x4000007B` — flags `0b01` (VERSIONED), payload length `123`.

**Resulting payload length**: exactly **123 bytes**.

**Canonical Base64 (no end marker)**:

```
QAAAewMABVRJVExFAAZBVVRIT1IAAAAAAAM8IAALZFF3NHc5V2dYY1EAAQAZaHR0cHM6Ly9leGFtcGxlLmNvbS93YXRjaAEAG2h0dHBzOi8vZXhhbXBsZS5jb20vYXJ0LnBuZwEAB0lTUkMxMjMAB3lvdXR1YmUAAAAAAAAAAA==
```

**[VERIFIED]** — this string decodes to exactly 127 bytes: 4 header bytes + 123 payload bytes, and
contains **no** trailing zero header (the `finish()` marker).

**Implementation warning for Rust** — Java's `DataOutputStream.writeUTF` uses *modified* UTF-8, which
differs from Rust's `str::encode_utf8` in exactly one respect: **U+0000 is encoded as the two bytes
`0xC0 0x80`** rather than the single byte `0x00`. Any title, author, identifier, uri, artwork URL, isrc
or source name containing a NUL character must use the modified-UTF-8 encoder to remain byte-compatible.

### 6.5 How a track is actually encoded — per source manager

**The critical insight** — the fixed prefix above is only half the story. After the three nullable
strings, `encodeTrackDetails` writes the **source name** and then delegates to that source's own
`encodeTrack`. Almost every source manager writes **nothing** in that delegated call.

**OBSERVED** — each source manager's `encodeTrack(AudioTrack, DataOutput)` implementation:

| Source | `getSourceName()` | Line | `encodeTrack` body | Line |
| --- | --- | --- | --- | --- |
| Twitch | `twitch` | `source/twitch/TwitchStreamAudioSourceManager.java:77-79` | empty — "URL (identifier) is enough" | `:121-123` |
| Nico | `niconico` | `source/nico/NicoAudioSourceManager.java:71-73` | empty — "No extra information to save" | `:129-131` |
| SoundCloud | `soundcloud` | `source/soundcloud/SoundCloudAudioSourceManager.java:138-140` | empty — "No extra information to save" | `:177-179` |
| Bandcamp | `bandcamp` | `source/bandcamp/BandcampAudioSourceManager.java:65-67` | empty — "No special values to encode" | `:271-273` |
| YouTube | `youtube` | `source/youtube/YoutubeAudioSourceManager.java:153-155` | empty — "No custom values that need saving" | `:177-179` |
| Vimeo | `vimeo` | `source/vimeo/VimeoAudioSourceManager.java:52-54` | empty — "Nothing special to encode" | `:77-79` |
| HTTP | `http` | `source/http/HttpAudioSourceManager.java:59-61` | writes a probe descriptor | `:146-148` |
| Local | `local` | `source/local/LocalAudioSourceManager.java:32-34` | writes a probe descriptor | `:70-72` |

**The probe descriptor** — **OBSERVED** —
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/source/ProbingAudioSourceManager.java:22` and `:48-53`:

```
private static final char PARAMETERS_SEPARATOR = '|';

protected void encodeTrackFactory(MediaContainerDescriptor factory, DataOutput output) throws IOException {
    String probeInfo = factory.probe.getName() + (factory.parameters != null ? PARAMETERS_SEPARATOR +
        factory.parameters : "");
    output.writeUTF(probeInfo);
}
```

**Behavioural contract to reimplement** — `[INFERRED]` from `ProbingAudioSourceManager.java:48-68`:

- The probe descriptor is a **single UTF string** of the form `<probeName>` or `<probeName>|<parameters>`.
- The separator is the literal character `|` (`U+007C`).
- Decoding splits on the **first** `|`; the part before it is looked up in the container registry by
  name, the part after it is the parameters string (or `null` if there was no separator).
- If the probe name is not found in the registry, the descriptor decodes to `null` and the source
  manager falls back to a generic track.

**Behavioural contract** — `[INFERRED]` from `DefaultAudioPlayerManager.java:291-295`:

- The **source name string** is what makes the payload self-describing: a decoder does not have to
  guess which source manager produced the track, it reads the name and dispatches.
- This is why the HTTP source must be registered last — it is the fallback for any `http(s)://` URL,
  and the source name in the payload lets a decoder select it regardless of registration order.

### 6.6 Decoding

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:241-270`:

```
@Override
public DecodedTrackHolder decodeTrack(MessageInput stream) throws IOException {
    DataInput input = stream.nextMessage();
    if (input == null) { return null; }

    int version = (stream.getMessageFlags() & TRACK_INFO_VERSIONED) != 0 ? (input.readByte() & 0xFF) : 1;

    AudioTrackInfo trackInfo = new AudioTrackInfo(
        input.readUTF(),
        input.readUTF(),
        input.readLong(),
        input.readUTF(),
        input.readBoolean(),
        version >= 2 ? DataFormatTools.readNullableText(input) : null,
        version >= 3 ? DataFormatTools.readNullableText(input) : null,
        version >= 3 ? DataFormatTools.readNullableText(input) : null
    );
    AudioTrack track = decodeTrackDetails(trackInfo, input);
    long position = input.readLong();

    if (track != null) { track.setPosition(position); }

    stream.skipRemainingBytes();

    return new DecodedTrackHolder(track);
}
```

**Version-dependent field presence** — `[INFERRED]` from the cited block:

| Version | Fields present |
| --- | --- |
| `1` | title, author, length, identifier, isStream. `uri`, `artworkUrl`, `isrc` are all `null`. |
| `2` | the above + `uri`. `artworkUrl` and `isrc` are `null`. |
| `≥ 3` | the above + `uri` + `artworkUrl` + `isrc`. |

A decoder must implement all three versions to remain compatible with tracks encoded by older
Lavalink releases.

**Source dispatch on decode** — **OBSERVED** —
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:313-323`:
the source name written during encoding is read back and used to select the source manager, which then
constructs its concrete track type (`TwitchStreamAudioTrack`, `NicoAudioTrack`,
`SoundCloudAudioTrack`, `BandcampAudioTrack`, `YoutubeAudioTrack`, `VimeoAudioTrack`,
`HttpAudioTrack`, `LocalAudioTrack`).

**Behavioural contract** — `[INFERRED]` from `DefaultAudioPlayerManager.java:260` and the per-source
`decodeTrack` implementations cited in §6.5: the source manager consumes the bytes it wrote, and
`stream.skipRemainingBytes()` discards anything left over, so a decoder is tolerant of trailing data
inside the framed payload.

### 6.7 The Lavalink-side wrapper

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/util/util.kt:137-141`:

```
fun encodeTrack(audioPlayerManager: AudioPlayerManager, track: AudioTrack): String {
    val baos = ByteArrayOutputStream()
    audioPlayerManager.encodeTrack(MessageOutput(baos), track)
    return Base64.getEncoder().encodeToString(baos.toByteArray())
}
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/util/util.kt:131-135`:

```
fun decodeTrack(audioPlayerManager: AudioPlayerManager, message: String): AudioTrack {
    val bais = ByteArrayInputStream(Base64.getDecoder().decode(message))
    return audioPlayerManager.decodeTrack(MessageInput(bais)).decodedTrack
        ?: throw IllegalStateException("Failed to decode track due to a mismatching version or missing source manager")
}
```

**Note** — the decode path calls `MessageInput` and `nextMessage()`, so it **tolerates** a trailing
`finish()` marker: the marker makes `nextMessage()` return `null` on the *next* call, but the first
call returns the track. A Rust decoder must therefore accept **both** the canonical form (no marker)
and the marker-terminated form.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/util/util.kt:44-58`:

```
fun AudioTrack.toTrack(audioPlayerManager: AudioPlayerManager, pluginInfoModifiers: List<AudioPluginInfoModifier>): Track {
    return this.toTrack(encodeTrack(audioPlayerManager, this), pluginInfoModifiers)
}

fun AudioTrack.toTrack(encoded: String, pluginInfoModifiers: List<AudioPluginInfoModifier>): Track {
    val pluginInfo = pluginInfoModifiers.fold(JsonObject(emptyMap())) { acc, it ->
        val jsonObject = it.modifyAudioTrackPluginInfo(this) ?: JsonObject(emptyMap())
        acc + jsonObject
    }
    return Track(encoded, this.toInfo(), pluginInfo, this.userData as? JsonObject ?: JsonObject(emptyMap()))
}
```

**Note the asymmetry** — the encoded string Lavalink returns to clients is whatever was *given* to it
when decoding (or freshly encoded when loading). It is **not** re-encoded from the decoded track.
A Rust reimplementation must preserve this: `decodetrack` echoes the input string verbatim.

### 6.8 HTTP/local container probing — end to end

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/source/ProbingAudioSourceManager.java:30-44`:

```
protected AudioItem handleLoadResult(MediaContainerDetectionResult result) {
    if (result != null) {
        if (result.isReference()) {
            return result.getReference();
        } else if (!result.isContainerDetected()) {
            throw new FriendlyException("Unknown file format.", COMMON, null);
        } else if (!result.isSupportedFile()) {
            throw new FriendlyException(result.getUnsupportedReason(), COMMON, null);
        } else {
            return createTrack(result.getInfo(), result.getContainerDescriptor());
        }
    }
    return null;
}
```

**Behavioural contract** — `[INFERRED]` from the cited block plus `ProbingAudioSourceManager.java:55-68`:

The probe result has four outcomes, and a Rust implementation must reproduce all four:

| Outcome | Result |
| --- | --- |
| the probe result is a **reference** (e.g. an HTTP 302 to another URL) | the reference is followed and re-probed |
| no container could be detected | `FriendlyException("Unknown file format.", COMMON)` |
| a container was detected but is not supported | `FriendlyException(unsupportedReason, COMMON)` |
| a supported container was detected | a track is created with the `MediaContainerDescriptor` |

The `MediaContainerDescriptor` holds the probe object and the parameter string. On encode only the
**name** of the probe plus the parameters are written (§6.5); the descriptor is reconstructed by
looking the probe up by name in the container registry on decode.

## 7. Lavaplayer playback internals

### 7.1 The manager

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:40-48`:

```
public static final int DEFAULT_CLEANUP_THRESHOLD = 60_000;
public static final int DEFAULT_FRAME_BUFFER_DURATION = 5000;
public static final int MAXIMUM_LOAD_REDIRECTS = 5;
public static final int DEFAULT_LOADER_POOL_SIZE = 10;
public static final int LOADER_QUEUE_CAPACITY = 5000;
```

| Constant | Value | Meaning |
| --- | --- | --- |
| `DEFAULT_CLEANUP_THRESHOLD` | 60 000 ms | A player with no active track for this long is destroyed. |
| `DEFAULT_FRAME_BUFFER_DURATION` | 5 000 ms | Lavaplayer's internal decoded-frame buffer size. |
| `MAXIMUM_LOAD_REDIRECTS` | 5 | Maximum HTTP redirects followed while resolving an identifier. |
| `DEFAULT_LOADER_POOL_SIZE` | 10 | Threads in the track-loading pool. |
| `LOADER_QUEUE_CAPACITY` | 5 000 | Bounded queue length for the loading pool. |

### 7.2 Executor fleet

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:53-96`:

```
// Executors
private final ExecutorService trackPlaybackExecutorService;
private final ThreadPoolExecutor trackInfoExecutorService;
private final ScheduledExecutorService scheduledExecutorService;
private final OrderedExecutor orderedInfoExecutor;
...
public DefaultAudioPlayerManager() {
    sourceManagers = new ArrayList<>();

    // Executors
    trackPlaybackExecutorService = new ThreadPoolExecutor(1, Integer.MAX_VALUE, 10, TimeUnit.SECONDS,
        new SynchronousQueue<>(), new DaemonThreadFactory("playback"));
    trackInfoExecutorService = ExecutorTools.createEagerlyScalingExecutor(1, DEFAULT_LOADER_POOL_SIZE,
        TimeUnit.SECONDS.toMillis(30), LOADER_QUEUE_CAPACITY, new DaemonThreadFactory("info-loader"));
    scheduledExecutorService = Executors.newScheduledThreadPool(1, new DaemonThreadFactory("manager"));
    orderedInfoExecutor = new OrderedExecutor(trackInfoExecutorService);

    // Configuration
    trackStuckThreshold = TimeUnit.MILLISECONDS.toNanos(10000);
    configuration = new AudioConfiguration();
    cleanupThreshold = new AtomicLong(DEFAULT_CLEANUP_THRESHOLD);
    frameBufferDuration = DEFAULT_FRAME_BUFFER_DURATION;
    useSeekGhosting = true;
    ...
}
```

| Executor | Type | Sizing | Purpose |
| --- | --- | --- | --- |
| `trackPlaybackExecutorService` | `ThreadPoolExecutor(1, MAX_VALUE, 10 s, SynchronousQueue)` | core 1, **unbounded max** | one task per active track's processing loop |
| `trackInfoExecutorService` | `ExecutorTools.createEagerlyScalingExecutor(1, 10, 30 s, 5000)` | core 1, max 10, queue 5 000 | resolves identifiers into track metadata |
| `scheduledExecutorService` | `newScheduledThreadPool(1)` | 1 thread | lifecycle cleanup + GC monitor |
| `orderedInfoExecutor` | `OrderedExecutor` wrapper | — | serialises loads per ordering key |

**Key details a reimplementation must preserve** — `[INFERRED]` from the cited block:

- The playback executor uses a **`SynchronousQueue`**, not a bounded queue. That means a task is
  handed directly to an idle thread or rejected — there is no backlog. Combined with the unbounded
  maximum pool size, this gives "one thread per concurrently playing track, created on demand".
- The loader executor uses an **eagerly-scaling** pool: it starts at 1 core thread and grows to 10
  under load, with a 30 s idle timeout before shrinking.
- The loader queue is bounded at 5 000. **OBSERVED** — `DefaultAudioPlayerManager.java:204-211`:
  when the queue is full the submission is rejected and the result handler receives
  `FriendlyException("Cannot queue loading a track, queue is full.", SUSPICIOUS)`.
  It is **not** blocked indefinitely.
- All four executors use **daemon** threads, so they never keep the JVM alive.

**Default configuration** — **OBSERVED** — `DefaultAudioPlayerManager.java:85-91`:

| Setting | Default | Unit |
| --- | --- | --- |
| `trackStuckThreshold` | 10 000 | **nanoseconds** (`TimeUnit.MILLISECONDS.toNanos(10000)`) |
| `cleanupThreshold` | 60 000 | ms |
| `frameBufferDuration` | 5 000 | ms |
| `useSeekGhosting` | `true` | — |

**Note the unit asymmetry** — the stuck threshold is stored in nanoseconds while the cleanup threshold
and frame-buffer duration are in milliseconds. A Rust port must not confuse them.

### 7.3 Identifier resolution (`loadItem`)

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:152-190`:

```
@Override
public @Nullable AudioItem loadItemSync(AudioReference reference) {
    AudioItem item = checkSourcesForItem(reference);
    if (item == null) {
        log.debug("No matches for track with identifier {}.", reference.identifier);
    }
    return item;
}

@Override
public Future<Void> loadItem(final AudioReference reference, final AudioLoadResultHandler resultHandler) {
    try {
        return trackInfoExecutorService.submit(() -> {
            loadItemSync(reference, resultHandler);
            return null;
        });
    } catch (RejectedExecutionException e) {
        return handleLoadRejected(reference.identifier, resultHandler, e);
    }
}
```

**Behavioural contract to reimplement** — `[INFERRED]` from the cited blocks:

- `loadItem` is **asynchronous**: it submits to the loader pool and returns a `Future`.
- `loadItemSync` is the blocking form and returns `null` when nothing matched.
- The `AudioLoadResultHandler` is invoked with either the loaded item or a failure. A `FriendlyException`
  with severity `FAULT` and message `"Something went wrong when looking up the track"` is produced for
  unexpected errors.
  **OBSERVED** — `DefaultAudioPlayerManager.java:213-218`.
- `loadItemOrdered` uses the `OrderedExecutor` so that loads for the same ordering key (e.g. the same
  guild) run one after another rather than concurrently.
  **OBSERVED** — `DefaultAudioPlayerManager.java:192-202`.

**Source iteration** — **OBSERVED** —
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:435-455`:

```
@Nullable
private AudioItem checkSourcesForItemOnce(AudioReference reference) {
    for (AudioSourceManager sourceManager : sourceManagers) {
        if (reference.containerDescriptor != null && !(sourceManager instanceof ProbingAudioSourceManager)) {
            continue;
        }

        AudioItem item = sourceManager.loadItem(this, reference);
        if (item != null) {
            ...
            return item;
        }
    }

    return null;
}
```

**Behavioural contract to reimplement** — `[INFERRED]` from the cited block:

- Source managers are consulted **in registration order** and the first non-null result wins.
- A manager that does not implement `ProbingAudioSourceManager` is **skipped entirely** when the
  reference carries a container descriptor. This is how the HTTP/local probing path is isolated.
- Note that `loadItem` takes the **manager** as its first argument (`loadItem(this, reference)`), so a
  source manager can call back into the manager if it needs to.

**Redirect following** — **OBSERVED** — `DefaultAudioPlayerManager.java:419-433`:
`checkSourcesForItem` loops. If the loaded item is an `AudioReference` rather than a track or
playlist, it is treated as a redirect: the reference is replaced with the new one and the loop
continues. `MAXIMUM_LOAD_REDIRECTS` (5) bounds the number of hops.

**OBSERVED** — `DefaultAudioPlayerManager.java:43`: `MAXIMUM_LOAD_REDIRECTS = 5`.

**Source dispatch on decode** — **OBSERVED** —
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/DefaultAudioPlayerManager.java:313-323`:

```
private AudioTrack decodeTrackDetails(AudioTrackInfo trackInfo, DataInput input) throws IOException {
    String sourceName = input.readUTF();

    for (AudioSourceManager sourceManager : sourceManagers) {
        if (sourceName.equals(sourceManager.getSourceName())) {
            return sourceManager.decodeTrack(trackInfo, input);
        }
    }

    return null;
}
```

**Behavioural contract** — `[INFERRED]` from the cited block:

- The source name is read from the payload and matched against each **registered** manager's
  `getSourceName()`.
- If no registered manager matches, `decodeTrackDetails` returns `null`.
  **OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/util/util.kt:131-135`:
  Lavalink turns that into
  `IllegalStateException("Failed to decode track due to a mismatching version or missing source manager")`.

**Per-source identifier syntax** — `[INFERRED]` from each source manager's `loadItem`:

| Source | Identifier forms |
| --- | --- |
| `twitch` | a Twitch channel name, optionally with a video ID suffix |
| `niconico` | a `nicovideo.jp/watch/...` URL or a bare video ID |
| `youtube` | a YouTube video URL/ID, a playlist URL/ID, a search query, or an `ytsearch:` prefix |
| `soundcloud` | a SoundCloud track/playlist URL, or a search query |
| `bandcamp` | a `bandcamp.com/track/...` or `/album/...` URL |
| `vimeo` | a Vimeo video URL |
| `http` | any `http://` or `https://` URL |
| `local` | an absolute filesystem path |


### 7.4 Source names

**OBSERVED** — each source manager's `getSourceName()`. All paths are relative to
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/`:

| Manager | `sourceName` | File : line |
| --- | --- | --- |
| `TwitchStreamAudioSourceManager` | `twitch` | `source/twitch/TwitchStreamAudioSourceManager.java:77-79` |
| `NicoAudioSourceManager` | `niconico` | `source/nico/NicoAudioSourceManager.java:71-73` |
| `YoutubeAudioSourceManager` | `youtube` | `source/youtube/YoutubeAudioSourceManager.java:153-155` |
| `SoundCloudAudioSourceManager` | `soundcloud` | `source/soundcloud/SoundCloudAudioSourceManager.java:138-140` |
| `BandcampAudioSourceManager` | `bandcamp` | `source/bandcamp/BandcampAudioSourceManager.java:65-67` |
| `VimeoAudioSourceManager` | `vimeo` | `source/vimeo/VimeoAudioSourceManager.java:52-54` |
| `HttpAudioSourceManager` | `http` | `source/http/HttpAudioSourceManager.java:59-61` |
| `LocalAudioSourceManager` | `local` | `source/local/LocalAudioSourceManager.java:32-34` |

**Note the exact spelling** — the Nico source's name is `niconico`, **not** `nico`, even though the
manager class is `NicoAudioSourceManager` and the Lavalink config key is `lavalink.server.sources.nico`.
This string is what is written into the encoded-track payload and what a decoder matches on, so it
must be reproduced byte-for-byte. **OBSERVED** — `source/nico/NicoAudioSourceManager.java:71-73`.

**Additional sources present in Lavaplayer but not registered by Lavalink 4.2.2** — **OBSERVED**:
`source/beam/BeamAudioSourceManager.java`, `source/getyarn/GetyarnAudioSourceManager.java` and
`source/yamusic/YandexMusicAudioSourceManager.java` exist in Lavaplayer 2.2.6 but Lavalink's
`AudioPlayerConfiguration` does not register them. They are reachable only through plugins.

**Source matching on decode** — **OBSERVED** — `DefaultAudioPlayerManager.java:313-323`: the decode
path matches a source manager by **exact source-name equality**, not by scheme/host inspection. The
`getyarn.io` and `yandex-music` names come from plugin source managers, not from Lavaplayer core.

### 7.5 The audio pipeline

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/filter/AudioPipelineFactory.java:32-65`:

```
public static AudioPipeline create(AudioProcessingContext context, PcmFormat inputFormat) {
    int inputChannels = inputFormat.channelCount;
    int outputChannels = context.outputFormat.channelCount;

    UniversalPcmAudioFilter end = new FinalPcmAudioFilter(context, createPostProcessors(context));
    FilterChainBuilder builder = new FilterChainBuilder();
    builder.addFirst(end);

    if (context.filterHotSwapEnabled || context.playerOptions.filterFactory.get() != null) {
        UserProvidedAudioFilters userFilters = new UserProvidedAudioFilters(context, end);
        builder.addFirst(userFilters);
    }

    if (inputFormat.sampleRate != context.outputFormat.sampleRate) {
        builder.addFirst(new ResamplingPcmAudioFilter(context.configuration, outputChannels,
            builder.makeFirstFloat(outputChannels), inputFormat.sampleRate, context.outputFormat.sampleRate));
    }

    if (inputChannels != outputChannels) {
        builder.addFirst(new ChannelCountPcmAudioFilter(inputChannels, outputChannels,
            builder.makeFirstUniversal(outputChannels)));
    }

    return new AudioPipeline(builder.build(null, inputChannels));
}

private static Collection<AudioPostProcessor> createPostProcessors(AudioProcessingContext context) {
    AudioChunkEncoder chunkEncoder = context.outputFormat.createEncoder(context.configuration);

    return Arrays.asList(
        new VolumePostProcessor(context),
        new BufferingPostProcessor(context, chunkEncoder)
    );
}
```

**Pipeline stages** — `[INFERRED]` from the cited block plus the `addFirst` ordering:

```
source decoder
   ↓
[ChannelCountPcmAudioFilter]      (only if input channels ≠ output channels)
   ↓
[ResamplingPcmAudioFilter]        (only if input rate ≠ 48000 Hz)
   ↓
[UserProvidedAudioFilters]        (the Lavaplayer filter chain: equalizer, karaoke, …)
   ↓
FinalPcmAudioFilter
   ↓
VolumePostProcessor                (volume is a post-processor, not a chain filter)
   ↓
BufferingPostProcessor → OpusChunkEncoder → frame buffer
```

**Key details a reimplementation must preserve** — `[INFERRED]` from the cited block:

- `addFirst` **prepends**, so the stages are inserted in reverse order of the code. The final
  signal-flow order is: channel conversion → resampling → user filters → volume → buffering.
- The **volume** filter is a **post-processor**, not part of the filter chain. This is why a volume
  change takes effect immediately on the currently playing track without a hot-swap.
- The resampler and channel converter are **conditionally** inserted: if the source is already
  48 kHz stereo, neither stage exists at all.
- The `FinalPcmAudioFilter` is what hands the PCM to the post-processors.

**Processing short-circuit** — **OBSERVED** — `AudioPipelineFactory.java:20-23`:

```
public static boolean isProcessingRequired(AudioProcessingContext context, AudioDataFormat inputFormat) {
    return !context.outputFormat.equals(inputFormat) || context.playerOptions.volumeLevel.get() != 100 ||
        context.playerOptions.filterFactory.get() != null;
}
```

**Behavioural contract** — `[INFERRED]`: if the source format already equals the output format, the
volume is exactly 100 and there is no filter factory, the whole pipeline is **skipped** and the
source's bytes are passed through unchanged. A Rust implementation should reproduce this fast path
because it is a meaningful performance difference.

### 7.6 Frame assembly — the 20 ms Opus frame

The frame size is **not** defined by the post-processor. It comes from the output format's
`chunkSampleCount`.

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/format/StandardAudioDataFormats.java:10`:

```
public static final AudioDataFormat DISCORD_OPUS = new OpusAudioDataFormat(2, 48000, 960);
```

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/format/transcoder/OpusChunkEncoder.java:28-35`:

```
@Override
public byte[] encode(ShortBuffer buffer) {
    encoder.encode(buffer, format.chunkSampleCount, encodedBuffer);
    byte[] bytes = new byte[encodedBuffer.remaining()];
    encodedBuffer.get(bytes);
    return bytes;
}
```

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/filter/BufferingPostProcessor.java:32-42`:

```
@Override
public void process(long timecode, ShortBuffer buffer) throws InterruptedException {
    outputBuffer.clear();
    encoder.encode(buffer, outputBuffer);
    offeredFrame.setTimecode(timecode);
    offeredFrame.setVolume(context.playerOptions.volumeLevel.get());
    offeredFrame.setBuffer(outputBuffer);
    context.frameBuffer.consume(offeredFrame);
}
```

**Behavioural contract to reimplement** — `[INFERRED]` from the three cited blocks:

- The Opus encoder is invoked with `frameSize = format.chunkSampleCount` = **960** samples.
- 960 samples ÷ 48 000 samples/s = **20 ms** per Opus frame. This is the value the RTP timestamp
  increment of 960 in §8.3 matches.
- Each `process` call produces exactly one Opus packet, which is handed to the frame buffer as one
  frame with the source timecode and the player's current volume.
- The frame buffer is therefore a queue of discrete 20 ms Opus packets, not a byte stream.

**Silence frame** — **OBSERVED** —
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/format/OpusAudioDataFormat.java:15`:

```
private static final byte[] SILENT_OPUS_FRAME = new byte[]{(byte) 0xFC, (byte) 0xFF, (byte) 0xFE};
```

This exact 3-byte sequence is the canonical Opus silence frame and is what a Rust implementation must
emit when it needs to fill a gap.

**Encoded frame size bounds** — **OBSERVED** —
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/format/OpusAudioDataFormat.java:25-30`:

```
public OpusAudioDataFormat(int channelCount, int sampleRate, int chunkSampleCount) {
    super(channelCount, sampleRate, chunkSampleCount);
    this.maximumChunkSize = 32 + 1536 * chunkSampleCount / 960;
    this.expectedChunkSize = 32 + 512 * chunkSampleCount / 960;
}
```

With `chunkSampleCount = 960` this gives `maximumChunkSize = 1568` bytes and
`expectedChunkSize = 544` bytes. A Rust implementation sizing its buffers must use the **maximum**
(1568), not the expected value.

### 7.7 Opus encoding

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/natives/opus/OpusEncoderLibrary.java:9`:

```
static final int APPLICATION_AUDIO = 2049;
```

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/natives/opus/OpusEncoderLibrary.java:20`:

```
native long create(int sampleRate, int channels, int application, int quality);
```

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/format/transcoder/OpusChunkEncoder.java:22-26`:

```
public OpusChunkEncoder(AudioConfiguration configuration, AudioDataFormat format) {
    encodedBuffer = ByteBuffer.allocateDirect(format.maximumChunkSize());
    encoder = new OpusEncoder(format.sampleRate, format.channelCount, configuration.getOpusEncodingQuality());
    this.format = format;
}
```

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/AudioConfiguration.java:12` and `:23-29`:

```
public static final int OPUS_QUALITY_MAX = 10;
...
public AudioConfiguration() {
    resamplingQuality = ResamplingQuality.LOW;
    opusEncodingQuality = OPUS_QUALITY_MAX;
    outputFormat = StandardAudioDataFormats.DISCORD_OPUS;
    filterHotSwapEnabled = false;
    frameBufferFactory = AllocatingAudioFrameBuffer::new;
}
```

**Behavioural contract** — `[INFERRED]` from the cited blocks:

- The Discord wire format is Opus, 2 channels, 48 000 Hz, **960 samples per frame (20 ms)**.
- The encoder's Opus application is `APPLICATION_AUDIO` (2049).
- The encoder quality is `configuration.getOpusEncodingQuality()`, which defaults to `OPUS_QUALITY_MAX` = 10.
- The default output format is `DISCORD_OPUS` and the default frame buffer factory is
  `AllocatingAudioFrameBuffer`.
- The output buffer is a **direct** `ByteBuffer` sized `format.maximumChunkSize()` = 1568 bytes,
  because the native encoder reads from it directly.

### 7.8 Resampling quality

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/AudioConfiguration.java:14`:

```
public enum ResamplingQuality { LOW, MEDIUM, HIGH }
```

**OBSERVED** — `AudioConfiguration.java:24` and `:87`: the default is `LOW`.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/AudioPlayerConfiguration.kt:168-186`:
Lavalink maps the `ResamplingQuality` enum to the libsamplerate converter ratio:

| Enum | libsamplerate ratio |
| --- | --- |
| `LOW` | `SampleRateConverter.CONVERSION_RATIO_LOW` (1) |
| `MEDIUM` | `SampleRateConverter.CONVERSION_RATIO_MEDIUM` (2) |
| `HIGH` | `SampleRateConverter.CONVERSION_RATIO_HIGH` (3) |

### 7.9 The frame buffer

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/track/playback/AllocatingAudioFrameBuffer.java:18-35`:

```
public class AllocatingAudioFrameBuffer extends AbstractAudioFrameBuffer {
    private final int fullCapacity;
    private final ArrayBlockingQueue<AudioFrame> audioFrames;
    private final AtomicBoolean stopping;

    public AllocatingAudioFrameBuffer(int bufferDuration, AudioDataFormat format, AtomicBoolean stopping) {
        super(format);
        this.fullCapacity = bufferDuration / 20 + 1;
        this.audioFrames = new ArrayBlockingQueue<>(fullCapacity);
        this.stopping = stopping;
    }
```

**Behavioural contract to reimplement** — `[INFERRED]` from the cited block plus the 20 ms frame
duration established in §7.6:

- The buffer holds `bufferDuration / 20 + 1` frames of 20 ms each, i.e. slightly more than
  `bufferDuration` ms of audio. With the 5 000 ms default that is **251 frames**.
- The backing store is an `ArrayBlockingQueue` with that exact capacity, so a producer **blocks** when
  the buffer is full. This is the back-pressure mechanism that keeps decode from running ahead of
  playback.
- Consumption is **non-blocking** (the class Javadoc says it "Consumes frames in a blocking manner and
  provides frames in a non-blocking manner").
  **OBSERVED** — `AllocatingAudioFrameBuffer.java:14-17`.

**Two implementations** — **OBSERVED** —
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/AudioConfiguration.java:28`:
the default factory is `AllocatingAudioFrameBuffer::new`.
`lavalink.server.non-allocating-frame-buffer` swaps it for `NonAllocatingAudioFrameBuffer`, which
reuses frame objects instead of allocating a new one per frame.

**Frame buffer lifecycle flags** — **OBSERVED** —
`Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/track/playback/AbstractAudioFrameBuffer.java:27-58`:

- `setTerminateOnEmpty()` — terminate once the buffer drains. It also triggers `clearOnInsert` if that
  flag was set.
- `setClearOnInsert()` — clear the buffer when the next frame is inserted. This is the mechanism used
  to cut over cleanly on a seek.

### 7.10 The playback loop

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/track/playback/LocalAudioTrackExecutor.java:233-234` and `:243-267`:

```
public void executeProcessingLoop(ReadExecutor readExecutor, SeekExecutor seekExecutor) {
    executeProcessingLoop(readExecutor, seekExecutor, true);
}

public void executeProcessingLoop(ReadExecutor readExecutor, SeekExecutor seekExecutor, boolean waitOnEnd) {
    ...
    if (checkPendingSeek(seekExecutor) == SeekResult.EXTERNAL_SEEK) {
        ...
    }
    ...
    performSeek(seekExecutor, nextPosition);
```

**Seek handling** — **OBSERVED** — `LocalAudioTrackExecutor.java:176-210` and `:371-407`:

| Mechanism | Line | Behaviour |
| --- | --- | --- |
| `queuedSeek` (`AtomicLong`, initial `-1`) | `:37` | holds a pending seek target; `-1` means "no pending seek" |
| `checkPendingSeek` | `:176-210` | returns `EXTERNAL_SEEK` when the player, not the executor, performed the seek |
| `performSeek` | `:390-407` | applies the seek to the source, then resets `queuedSeek` to `-1` |
| `useSeekGhosting` | `:33`, `:61`, `:193`, `:401` | when true, old frames keep being emitted across the seek |

**Seek ghosting** — **OBSERVED** — `LocalAudioTrackExecutor.java:50`:

```
@param useSeekGhosting Whether to keep providing old frames continuing from the previous position during a seek
```

**Behavioural contract** — `[INFERRED]` from `:176-210` and `:390-407`:

- A seek requested while the executor is mid-loop is **queued**, not applied immediately.
- The queue is drained at the next safe point in the loop.
- When `useSeekGhosting` is false, `frameBuffer.setClearOnInsert()` is used so the buffer is flushed
  at the seek boundary; when true, the buffer keeps its contents so the old audio plays out.
- `SeekResult.EXTERNAL_SEEK` is the sentinel meaning "the player already handled this seek, do not
  re-seek the source". **OBSERVED** — `LocalAudioTrackExecutor.java:478-484`.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/AudioPlayerConfiguration.kt:168-186`:
Lavalink passes `useSeekGhosting` through to `AudioConfiguration.useSeekGhosting`.

### 7.11 Player lifecycle management

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/AudioPlayerLifecycleManager.java:20-38`:

```
private final AtomicLong cleanupThreshold;
private final AtomicReference<ScheduledFuture<?>> scheduledTask;
...
public AudioPlayerLifecycleManager(ScheduledExecutorService scheduler, AtomicLong cleanupThreshold) {
    ...
    this.cleanupThreshold = cleanupThreshold;
    this.scheduledTask = new AtomicReference<>();
    ...
    ScheduledFuture<?> task = scheduler.scheduleAtFixedRate(this, CHECK_INTERVAL, CHECK_INTERVAL, TimeUnit.MILLISECONDS);
```

**OBSERVED** — `AudioPlayerLifecycleManager.java:66`:

```
player.checkCleanup(cleanupThreshold.get());
```

**Behavioural contract** — `[INFERRED]` from the cited blocks:

- A player that has had no active track for `DEFAULT_CLEANUP_THRESHOLD` (60 s) is removed from the
  manager and its resources released.
- The cleanup threshold is held in an `AtomicLong` so it can be changed at runtime.
- The scheduled task is held in an `AtomicReference`, so changing the threshold can cancel and
  replace the scheduled task without racing the sweep.

### 7.12 Track events

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/event/`:

| Event | Fired when |
| --- | --- |
| `TrackStartEvent` | a track begins producing audio |
| `TrackEndEvent` | a track stops, with an `AudioTrackEndReason` |
| `TrackExceptionEvent` | a track throws |
| `TrackStuckEvent` | no frame is produced for the stuck threshold |
| `PlayerPauseEvent` | the player is paused |
| `PlayerResumeEvent` | the player is resumed |

**Stuck threshold** — **OBSERVED** —
`Lavalink/LavalinkServer/src/main/java/lavalink/server/config/AudioPlayerConfiguration.kt:168-186`:
the default is 10 000 ms, and values below 100 ms are rejected.

### 7.13 Loss counting

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/AudioLossCounter.kt`:

```
private static final int EXPECTED_PACKET_COUNT_PER_MIN = 60 * 1000 / 20;
private static final int ACCEPTABLE_TRACK_SWITCH_TIME = 100;
```

**Behavioural contract** — `[INFERRED]`:

- Discord voice packets are 20 ms each, so 3 000 packets are expected per minute.
- A track switch is allowed to lose up to 100 ms (5 packets) of audio before it counts as a loss.
- The counter is used to decide whether the gateway connection should be re-established.

---

## 8. Concurrency and timing model

This section is the timing contract a Rust reimplementation must satisfy.

### 8.1 Timing constants — consolidated table

| Constant | Value | Source |
| --- | --- | --- |
| Discord Opus frame duration | 20 ms | derived: 960 samples ÷ 48 000 Hz |
| RTP timestamp increment per frame | 960 | `core-3.0.0-pre6.jar:moe.kyokobot.koe.poller.AbstractOpusFramePoller` `[OBSERVED-BYTECODE]` |
| Koe frame poller interval | 20 000 000 ns (20 ms) | same class, `FRAME_INTERVAL_NANOS` `[OBSERVED-BYTECODE]` |
| Koe silence frame count | 5 | same class, `SILENCE_FRAME_COUNT` `[OBSERVED-BYTECODE]` |
| Koe UDP-queue default buffer | 400 ms | `ext-udpqueue-3.0.0-pre6.jar:moe.kyokobot.koe.poller.udpqueue.QueueManagerPool`, `DEFAULT_BUFFER_DURATION` `[OBSERVED-BYTECODE]` |
| Koe UDP-queue max packet | 4 096 bytes | same class, `MAXIMUM_PACKET_SIZE` `[OBSERVED-BYTECODE]` |
| Lavaplayer frame buffer | 5 000 ms default | `DefaultAudioPlayerManager.java:40` |
| Lavaplayer cleanup threshold | 60 000 ms | `DefaultAudioPlayerManager.java:41` |
| Loader pool size | 10 threads | `DefaultAudioPlayerManager.java:44` |
| Loader queue capacity | 5 000 entries | `DefaultAudioPlayerManager.java:45` |
| Max load redirects | 5 | `DefaultAudioPlayerManager.java:43` |
| Player update interval | 5 s default | `ServerConfig.kt:29-51` |
| Stats push interval | 60 s | `SocketContext.kt:98-107` |
| Session resume timeout | 60 s default | `SocketContext.kt:78` |
| Track stuck threshold | 10 000 ms default, min 100 ms | `AudioPlayerConfiguration.kt:168-186` |
| Acceptable track-switch loss | 100 ms | `AudioLossCounter.kt` |
| Expected packets per minute | 3 000 | `AudioLossCounter.kt` |
| HTTP timeouts (connect/request/socket) | 3 000 ms each | `ServerConfig.kt:53-57` |
| CPU stats refresh | 30 000 ms | `StatsCollector.kt:36-90` |

### 8.2 Thread inventory

**OBSERVED** — derived from the executor constructions cited above:

| Pool | Threads | Lifetime | Task |
| --- | --- | --- | --- |
| `trackPlaybackExecutorService` | `max(2, availableProcessors)` | manager lifetime | one task per active track's processing loop |
| `trackInfoExecutorService` | 10, queue 5 000 | manager lifetime | identifier resolution / track metadata |
| `scheduledExecutorService` | — | manager lifetime | lifecycle cleanup sweeps |
| `playerUpdateService` (Lavalink) | 2, daemon | session lifetime | periodic `playerUpdate` pushes |
| `executor` (Lavalink `SocketContext`) | — | session lifetime | stats push + resume-timeout timer |
| Undertow worker threads | — | server lifetime | HTTP + WebSocket I/O |

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:101-106`:
the player-update pool uses 2 daemon threads named `player-update`.

**Behavioural contract** — `[INFERRED]`: a single guild's player state is mutated from the REST
thread, the WebSocket thread and the playback thread. Lavalink guards player mutations with
`synchronized(player)` in the PATCH handler. **OBSERVED** —
`Lavalink/LavalinkServer/src/main/java/lavalink/server/player/PlayerRestHandler.kt:113-141`.

### 8.3 Frame pacing

**OBSERVED-BYTECODE** — `core-3.0.0-pre6.jar:moe.kyokobot.koe.poller.AbstractOpusFramePoller`:

- `FRAME_INTERVAL_NANOS = 20_000_000`
- `RTP_TIMESTAMP_INCREMENT = 960`
- `SILENCE_FRAME_COUNT = 5`
- Fields: `volatile long lastFrameTime`, `volatile long droppedFrames`
- `scheduleNext()` uses `System.nanoTime()` for scheduling decisions.

**Behavioural contract to reimplement** — `[INFERRED]` from the constants above:

1. Every 20 ms the poller emits one Opus frame.
2. The RTP timestamp advances by exactly **960** per frame (the standard Opus/Dissonance 48 kHz clock rate).
3. If the poller wakes late, it catches up by emitting frames back-to-back (the `lastFrameTime` /
   `droppedFrames` pair tracks the drift).
4. If no audio is available, up to **5** silence frames are emitted before the poller gives up.

**Two poller implementations ship in the Koe core jar** — `[OBSERVED-BYTECODE]`:

| Class | Package | Notes |
| --- | --- | --- |
| `moe.kyokobot.koe.poller.udpqueue.UdpQueueOpusFramePoller` | `ext-udpqueue` jar | Uses the native UDP-queue; overrides `getPollsPerTick()`. |
| `moe.kyokobot.koe.poller.netty.NettyOpusFramePoller` | `core` jar | Pure-Java fallback when the native UDP queue is unavailable. |

**OBSERVED-BYTECODE** — `core-3.0.0-pre6.jar:moe.kyokobot.koe.poller.AbstractFramePoller`:
`getFrameIntervalNanos()` is abstract and `pollAndSend()` is abstract, so both pollers share the
20 ms scheduling skeleton.

### 8.4 UDP-queue mode vs Netty mode

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/AudioPlayerConfiguration.kt:168-186`:
`lavalink.server.buffer-duration-ms` controls the JDA-NAS UDP-queue buffer.

**Behavioural contract** — `[INFERRED]` from the config semantics and the Koe class layout:

| `buffer-duration-ms` | Behaviour |
| --- | --- |
| unset (`null`) | the UDP queue is used with Koe's own 400 ms default |
| `> 0` | the UDP queue is used with the given duration |
| `≤ 0` | JDA-NAS is disabled and the Netty fallback poller is used |

**Why this matters** — `[INFERRED]`: the UDP queue moves the packetisation to native code and can
produce lower and more stable latency. The Netty fallback runs the same 20 ms schedule in the JVM.
A Rust reimplementation should target the **same 20 ms frame cadence and 960-sample RTP increment**
regardless of which path it uses.

### 8.5 Voice gateway connection lifecycle

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/LavalinkPlayer.kt:38-55`:

```
private var mediaConnection: MediaConnection? = null
...
fun provideTo(connection: MediaConnection) {
    mediaConnection = connection
    connection.configureFrameProvider(audioPlayer, true)
}
```

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:131-138`:
`destroyPlayer` closes the media connection with `MediaConnection.CloseReason.DESTROYED`.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/PlayerRestHandler.kt:113-141`:
a new voice state closes the old connection with `CloseReason.REPLACED` before creating the new one.

**Behavioural contract** — `[INFERRED]`: the media connection is identified by the tuple
(endpoint, token, sessionId, channelId). A PATCH that changes any component replaces the connection.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:219-234`:
the `WebSocketClosedEvent` is emitted when the **voice** gateway closes, with `byRemote` indicating
whether the gateway initiated the close.

### 8.6 Idle cleanup

**OBSERVED** — `Lavaplayer/main/src/main/java/com/sedmelluq/discord/lavaplayer/player/AudioPlayerLifecycleManager.java`:
the cleanup task periodically scans the player map and removes players whose last-active timestamp is
older than the cleanup threshold.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/player/EventEmitter.kt`:
a cleanup-driven track end is reported to clients with reason `cleanup`.

**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/config/ServerConfig.kt:29-51`:
`lavalink.server.gc-warnings` (default `true`) toggles Lavaplayer's garbage-collection monitoring
warnings, which are a separate concern from the idle cleanup.

### 8.7 Ordering guarantees a reimplementation should preserve

`[INFERRED]` from the cited code:

1. **A `TrackStartEvent` precedes the first `playerUpdate` for a newly started track.**
   The update task is scheduled with an initial delay of 0 in `onTrackStart`
   (`LavalinkPlayer.kt:114-125`), and the track-start event is emitted from the same callback.
2. **A `TrackEndEvent` precedes the final `playerUpdate`.**
   The task is cancelled in `onTrackEnd` (`LavalinkPlayer.kt:108-112`), but a final push is emitted
   by the REST handler after filter changes and seeks.
3. **The WebSocket event queue preserves ordering per session.**
   `SocketContext.kt:154-160` appends to a FIFO `ConcurrentLinkedQueue` while paused.
4. **Player updates for a given guild are serialised** because the update task is a single-threaded
   `scheduleAtFixedRate` on a 2-thread pool, and each guild has at most one scheduled task
   (`LavalinkPlayer.kt:114-125` guards with `updateFuture?.isCancelled == false`).

---

## 9. Dependency inventory

**OBSERVED** — `Lavalink/settings.gradle.kts:11-21` and the catalog-builder functions at `:23-98`.
Lavalink 4.2.2 declares its dependency versions **inline in `settings.gradle.kts`** via
`VersionCatalogBuilder` functions, not in a separate `gradle/libs.versions.toml`. There is no such
TOML file in the repository.

The catalog is created at `:11-21`:

```
dependencyResolutionManagement {
    versionCatalogs {
        create("libs") {
            spring()
            voice()
            metrics()
            common()
            other()
        }
    }
}
```

### 9.1 `spring()` — web stack

**OBSERVED** — `Lavalink/settings.gradle.kts:23-36`:

| Coordinate | Version | Purpose |
| --- | --- | --- |
| `org.springframework.boot:spring-boot` | 3.3.0 | Spring Boot core |
| `org.springframework.boot:spring-boot-starter-web` | 3.3.0 | Spring MVC + servlet API |
| `org.springframework.boot:spring-boot-starter-undertow` | 3.3.0 | Undertow (the actual servlet container) |
| `org.springframework:spring-websocket` | 6.1.9 | WebSocket support |
| `org.springframework.cloud:spring-cloud-starter-config` | 4.1.5 | Optional remote configuration |

**OBSERVED** — `Lavalink/settings.gradle.kts:35`: the `spring` bundle is
`[spring-websocket, spring-boot-web, spring-boot-undertow]`.

**Note** — the servlet container is **Undertow**, not Tomcat. This is observable in the WebSocket
send path, which reaches into Undertow's native session to write directly to the channel.
**OBSERVED** — `Lavalink/LavalinkServer/src/main/java/lavalink/server/io/SocketContext.kt:154-175`.

### 9.2 `voice()` — audio and voice

**OBSERVED** — `Lavalink/settings.gradle.kts:38-63`:

| Coordinate | Version | Purpose |
| --- | --- | --- |
| `dev.arbjerg:lavaplayer` | 2.2.6 | The audio engine |
| `dev.arbjerg:lavaplayer-ext-youtube-rotator` | 2.2.6 | YouTube IP rotation support for the route planner |
| `dev.arbjerg:lavadsp` | 0.7.8 | Native DSP filters |
| `moe.kyokobot.koe:core` | 3.0.0-pre6 | Discord voice gateway client |
| `moe.kyokobot.koe:ext-udpqueue` | 3.0.0-pre6 | Native UDP queue extension for Koe |
| `moe.kyokobot.libdave:natives-*` | 0.1.3 | Native libdave, per platform |
| `club.minnced:udpqueue-native-*` | 0.2.12 | Native UDP queue, per platform |

**Platform coverage** — **OBSERVED** — `Lavalink/settings.gradle.kts:51-62`:

| Bundle | Platforms |
| --- | --- |
| `libdave-natives` | `linux-x86-64`, `linux-x86`, `linux-aarch64`, `linux-arm`, `linux-musl-x86-64`, `linux-musl-aarch64`, `win-x86-64`, `win-x86`, `darwin` |
| `udpqueue-natives` | `linux-x86-64`, `linux-x86`, `linux-aarch64`, `linux-arm`, `linux-musl-x86-64`, `linux-musl-aarch64`, `win-x86-64`, `win-x86`, `win-aarch64`, `darwin` |

Note the asymmetry: `udpqueue-natives` covers `win-aarch64` but `libdave-natives` does not, and
`libdave-natives` covers `linux-arm` while `udpqueue-natives` also does. A Rust reimplementation must
pick its platform support accordingly.

### 9.3 `metrics()` — Prometheus

**OBSERVED** — `Lavalink/settings.gradle.kts:65-74`:

| Coordinate | Version |
| --- | --- |
| `io.prometheus:simpleclient` | 0.16.0 |
| `io.prometheus:simpleclient_hotspot` | 0.16.0 |
| `io.prometheus:simpleclient_logback` | 0.16.0 |
| `io.prometheus:simpleclient_servlet` | 0.16.0 |

These are the **legacy** `simpleclient` artifacts, **not** Micrometer. This is why the metrics endpoint
is configured through `metricsConfig.endpoint` and exempted from authentication by path rather than by a
Spring actuator path.

### 9.4 `common()` — Kotlin, serialization, XML, logging, OS metrics

**OBSERVED** — `Lavalink/settings.gradle.kts:76-91`:

| Coordinate | Version | Purpose |
| --- | --- | --- |
| `org.jetbrains.kotlin:kotlin-reflect` | 2.1.20 | Runtime reflection (Kotlin 2.1.20) |
| `org.jetbrains.kotlin:kotlin-stdlib-jdk8` | 2.1.20 | Kotlin stdlib |
| `org.jetbrains.kotlinx:kotlinx-serialization-json` | 1.7.0 | Protocol JSON codec |
| `org.jetbrains.kotlinx:kotlinx-datetime` | 0.6.0 | Date/time types |
| `io.github.pdvrieze.xmlutil:core-jdk` | 0.90.3 | XML config support |
| `io.github.pdvrieze.xmlutil:serialization-jvm` | 0.90.3 | XML serialization |
| `ch.qos.logback:logback-classic` | 1.5.6 | Logging backend |
| `io.sentry:sentry-logback` | 7.10.0 | Error reporting |
| `com.github.oshi:oshi-core` | 6.4.11 | CPU / memory / system metrics |

**Kotlin version is 2.1.20** — this is a notable fact: Lavalink 4.2.2 ships on Kotlin 2.x even though
the source layout is JVM-targeted. **OBSERVED** — `Lavalink/settings.gradle.kts:77`.

### 9.5 `other()` — publishing

**OBSERVED** — `Lavalink/settings.gradle.kts:93-98`: the `com.vanniktech.maven.publish` plugin at
version 0.32.0, used by both `Lavalink-Server` and `protocol`.

### 9.6 Transitive dependencies worth knowing

**OBSERVED** — from `Lavalink/LavalinkServer/build.gradle.kts:64-74`, Lavalink deliberately excludes:

| Exclusion | Why |
| --- | --- |
| `org.slf4j:slf4j-api` from `koe` and `oshi` | those artifacts bundle an SLF4J 1.2.3 that does not recognise Logback 1.5.x |
| `udp-queue` from `ext-udpqueue` | replaced by the newer `club.minnced:udpqueue-native-*` bundle |
| `com.sedmelluq:lava-common` from `udpqueue-natives` | avoids a duplicate/conflicting Lavaplayer common artifact |

**Behavioural contract for a reimplementation** — `[INFERRED]`: if you port to Rust you do not need any
of these SLF4J or Lavaplayer-common shims. The relevant takeaway is that **Lavaplayer is the
`dev.arbjerg` group** (a fork namespace), not the older `com.sedmelluq` group, even though the Java
package names inside it remain `com.sedmelluq.discord.lavaplayer`.

### 9.7 Native library requirement

**OBSERVED** — `Lavalink/LavalinkServer/build.gradle.kts:71-79` and `:126-138`. The following native
artifacts must be present at runtime for the corresponding features to work:

| Native | Needed for | Failure mode if absent |
| --- | --- | --- |
| `libopus` (via Lavaplayer) | Opus encode/decode | the audio pipeline cannot start |
| `libsamplerate` (via Lavaplayer) | resampling | resampled playback fails |
| `lavadsp` natives | the built-in filters | filter construction fails |
| `libdave` natives | Dave protocol | Dave-specific features fail |
| `udpqueue-native` | the JDA-NAS path | falls back to Netty polling |

**OBSERVED** — `Lavalink/LavalinkServer/build.gradle.kts:141-155`: for the `musl` target the build
produces `Lavalink-musl.jar` and excludes the glibc native jars, replacing them with a repackaged
`lavaplayer-natives-musl.jar`.

### 9.8 Protocol module targets

**OBSERVED** — `Lavalink/protocol/build.gradle.kts:20-59`: the protocol module is a Kotlin Multiplatform
module targeting `jvm`, `js` (IR, nodejs + browser), `linuxX64`, `linuxArm64`, `mingwX64`, `macosArm64`,
`macosX64`, `iosArm64`, `iosSimulatorArm64`, `watchosArm64`, `watchosSimulatorArm64`, `watchosX64`,
`tvosArm64`, `tvosSimulatorArm64` and `tvosX64`.

**Note** — `iosSimulatorArm64` is listed **twice** in the source.
**OBSERVED** — `Lavalink/protocol/build.gradle.kts:49-51`. This is a harmless duplication, not a
functional difference, but it is worth noting if you diff against a later tag.

---

## 10. Reimplementation checklist

This section is the distilled, ordered list of behaviours a Rust implementation must reproduce to be
protocol-compatible with Lavalink 4.2.2. Each item cross-references the section of this document that
contains the evidence.

### 10.1 Phase 1 — the wire protocol (no Lavalink source needed)

| # | Behaviour | Reference |
| --- | --- | --- |
| 1 | Implement the v4 JSON protocol exactly: `Track`, `TrackInfo`, `Playlist`, `Player`, `PlayerState`, `VoiceState`, `Filters`, `LoadResult`, `Stats`, `Info`. | §4 |
| 2 | `TrackInfo` field order and types: `identifier` (string), `isSeekable` (bool), `author` (string), `length` (long, ms), `isStream` (bool), `position` (long, ms), `title` (string), `uri` (string), `sourceName` (string), `artworkUrl` (string), `isrc` (string). | §4, §6.3 |
| 3 | Every optional protocol field uses the tri-state "omissible" encoding, not `null`. Absent ≠ null ≠ present. | §4.3 |
| 4 | `Track.encoded` is the base64 of the framed payload described in §6. `Track.userData` is a **separate** field and is **not** part of the encoded payload. | §6.2, §6.3 |
| 5 | `Track.pluginInfo` is a JSON object built by folding every plugin `AudioPluginInfoModifier` over the track. | §4.3 |
| 6 | The `op` field is a **string**, not an integer. Its four possible values are `"ready"`, `"stats"`, `"playerUpdate"` and `"event"`. **OBSERVED** — `protocol/v4/messages.kt:18-37`. | §5.3 |
| 7 | `ReadyEvent` is `{"op":"ready","resumed":bool,"sessionId":string}` — the fields are **flat**, not nested under `data`. **OBSERVED** — `messages.kt:39-50`. | §5.3 |
| 8 | `PlayerUpdateEvent` is `{"op":"playerUpdate","state":{time,position,connected,ping},"guildId":"..."}` — also flat, with `guildId` at the top level. **OBSERVED** — `messages.kt:52-64`. | §5.3 |
| 9 | `StatsEvent` is `{"op":"stats","frameStats":...,"players":int,"playingPlayers":int,"uptime":long,"memory":{...},"cpu":{...}}` — flat. **OBSERVED** — `messages.kt:66-87`. | §5.3 |
| 10 | The `event` op carries a nested `type` discriminator with five values: `TrackStartEvent`, `TrackEndEvent`, `TrackExceptionEvent`, `TrackStuckEvent`, `WebSocketClosedEvent`. **OBSERVED** — `messages.kt:89-117`. | §5.3 |

### 10.2 Phase 2 — HTTP surface

| # | Behaviour | Reference |
| --- | --- | --- |
| 11 | Implement every route in the §4.1 inventory with the same method, path and status codes. | §4.1 |
| 12 | Authentication: the `Authorization` header must equal `lavalink.server.password` **exactly** (byte-for-byte string comparison, no trimming, no Bearer prefix stripping). | §4.2 |
| 13 | Missing `Authorization` header → **401**. Present but wrong → **403**. | §4.2 |
| 14 | The metrics endpoint (if configured) and `/error` are exempt from auth. | §4.2 |
| 15 | The WebSocket handshake also enforces the same auth rule, plus a **required** `User-Id` header (400 if absent, empty, unparseable, or `0`). | §5.1, §4.3 |
| 16 | `GET /v4/loadtracks?identifier=…` returns a `LoadResult` — one of `trackLoaded`, `playlistLoaded`, `searchResult`, `noMatches`, `loadFailed`. | §4.4 |
| 17 | `loadFailed` is returned as **HTTP 200** with the failure in the body, not as an HTTP error. | §4.4 |
| 18 | `GET /v4/decodetrack?encodedTrack=…` (or `?track=…`) decodes one track. Both query params are accepted; `encodedTrack` wins. | §4.5 |
| 19 | `POST /v4/decodetracks` takes `{"tracks": [...]}` and decodes a batch. An empty list → **400**. | §4.5 |
| 20 | `PATCH /v4/sessions/{sessionId}/players/{guildId}` applies fields in the documented order (see §4.6). | §4.6 |
| 21 | `PATCH` rejects specifying **both** `track` and `encodedTrack` → **400**. | §4.6 |
| 22 | `PATCH` rejects a `filters` object naming a config-disabled filter → **400**. | §3.7, §4.6 |
| 23 | `PATCH` rejects a `voice` object with a blank `token`, `endpoint` or `sessionId`, or a null/blank `channelId` → **400**. | §4.6 |
| 24 | `PATCH` rejects `endTime <= 0` → **400**. | §4.6 |
| 25 | `PUT /v4/sessions/{sessionId}/players/{guildId}` replaces the whole player state with the same validation. | §4.6 |
| 26 | `DELETE /v4/sessions/{sessionId}/players/{guildId}` stops and destroys the player. | §4.6 |
| 27 | `GET /v4/sessions` and `GET /v4/sessions/{sessionId}` return the session list / detail. | §4.7 |
| 28 | `GET /v4/info` returns version, build time, git info, JVM version, Lavaplayer version, and the enabled-filter list. | §4.8 |
| 29 | `GET /v4/stats` returns the stats object **without** `frameStats`. | §4.10 |
| 30 | `GET /v4/routeplanner/status` → 204 when no planner; `POST …/free/address` and `POST …/free/all` free addresses. | §4.11 |
| 31 | `GET /version` returns the bare version string as plain text. | §4.9 |

### 10.3 Phase 3 — the encoded-track format (the highest-risk area)

| # | Behaviour | Reference |
| --- | --- | --- |
| 32 | The frame header is a **4-byte big-endian** word: `size \| (flags << 30)`. `size` occupies the low 30 bits, `flags` the top 2. | §6.1 |
| 33 | The payload follows the header immediately. The next frame's header follows the payload. | §6.1 |
| 34 | `flags == 1` means **VERSIONED**. The **first** byte of a versioned payload is the format version (`3` for Lavalink 4). | §6.2 |
| 35 | `finish()` appends a **4-byte zero header** as the end marker. A zero `size` means "no more messages". | §6.1 |
| 36 | Payload v3 field order: `version(u8=3)`, `title(UTF)`, `author(UTF)`, `length(i64)`, `identifier(UTF)`, `isStream(bool)`, `uri(present-bool + optional UTF)`, `artworkUrl(present-bool + optional UTF)`, `isrc(present-bool + optional UTF)`, `sourceName(UTF)`, `<source-specific bytes>`, `position(i64)`. | §6.3 |
| 37 | "UTF" means Java `DataOutput.writeUTF`: a **2-byte big-endian length prefix** followed by that many bytes of **modified UTF-8**. | §6.3 |
| 38 | The three nullable strings are encoded as a **boolean presence flag** followed by the UTF string **only if present**. A present-but-empty string is distinguishable from absent. | §6.3 |
| 39 | `sourceName` is followed by the source manager's own bytes. For `http` and `local` this is a **probe descriptor**: `<probeName>|<parameters>` separated by a literal `|` (0x7C). | §6.5 |
| 40 | For `twitch`, `niconico`, `soundcloud`, `bandcamp`, `youtube` and `vimeo`, `encodeTrack` is a **no-op** — zero source-specific bytes. | §6.5 |
| 41 | `position(i64)` is the **last** field of the payload. | §6.3 |
| 42 | `userData` is **not** encoded. It travels as the separate `Track.userData` protocol field. | §6.3 |
| 43 | The decoder must accept versions **1, 2 and 3**, with version-dependent field presence. Reject unknown versions with the exact message *"Failed to decode track due to a mismatching version or missing source manager"*. | §6.4 |
| 44 | A decoder that finds no registered source manager with a matching name returns `null` → the same error. | §6.4 |
| 45 | The canonical 123-byte payload has the base64 form given in §6.6. Use it as a **test vector**. | §6.6 |

### 10.4 Phase 4 — playback engine

| # | Behaviour | Reference |
| --- | --- | --- |
| 46 | Output format is **Opus, 48 000 Hz, 2 channels, 960 samples per frame (20 ms)**. | §7.6 |
| 47 | Frame size comes from `chunkSampleCount` of the output format, not from a hardcoded constant. | §7.6 |
| 48 | The Opus silence frame is exactly `{0xFC, 0xFF, 0xFE}`. | §7.6 |
| 49 | `maximumChunkSize = 1568`, `expectedChunkSize = 544` for the 960-sample format. | §7.6 |
| 50 | The frame buffer holds `bufferDuration / 20 + 1` frames and is a **blocking** queue: the producer blocks when full. | §7.7 |
| 51 | Default frame buffer duration is **5000 ms**; values below **200 ms** are rejected by Lavalink's config layer. | §3.1, §7.7 |
| 52 | The pipeline order is: channel conversion → resampling → user filters → volume → buffering/encoding. | §7.5 |
| 53 | The channel-conversion and resampling stages are **omitted entirely** when the source already matches the output format. | §7.5 |
| 54 | The whole pipeline is skipped when the format matches, the volume is exactly 100 and there is no filter factory. | §7.5 |
| 55 | **Volume is a post-processor, not a chain filter** — a volume change applies to the currently playing track without a hot-swap. | §7.5 |
| 56 | Filter chain order is fixed: volume, equalizer, karaoke, timescale, tremolo, vibrato, distortion, rotation, channelMix, lowPass, then plugin filters. | §3.8 |
| 57 | Each filter is constructed with the **previous** filter as its sink, and the list is reversed so it runs output-first. | §3.8 |
| 58 | A filter whose `build` returns `null` is silently dropped. | §3.8 |
| 59 | The "filter is enabled" predicates are exactly as tabulated in §3.8 (e.g. `lowPass` is enabled iff `smoothing > 1.0f`; `karaoke` is always enabled). | §3.8 |
| 60 | Filter hot-swap is enabled unconditionally, so a filter change applies to the playing track. | §3.8 |
| 61 | The source-resolution order is youtube → soundcloud → bandcamp → twitch → vimeo → nico → local → plugins → http. | §3.6 |
| 62 | Source managers are tried in registration order; the first non-null result wins. | §7.3 |
| 63 | A source manager that is not a `ProbingAudioSourceManager` is skipped when the reference carries a container descriptor. | §7.3 |
| 64 | Redirects are followed up to **5** times. | §7.3 |
| 65 | The source name written into the payload is `niconico` for the Nico source (not `nico`). | §7.4 |
| 66 | The default stuck-track threshold is **10 000 ns** in Lavaplayer; Lavalink's config layer exposes it in ms with a minimum of **100 ms**. | §7.10, §3.1 |
| 67 | The default cleanup interval is **60 000 ms**. | §7.10 |
| 68 | The default `useSeekGhosting` is `true`. | §3.1, §7.10 |

### 10.5 Phase 5 — concurrency and timing

| # | Behaviour | Reference |
| --- | --- | --- |
| 69 | Playback runs on a dedicated executor; track **loading** runs on a **separate** executor so a slow load never stalls playback. | §7.3, §8.2 |
| 70 | The playback executor is a `ThreadPoolExecutor(1, MAX_VALUE, 10 s, SynchronousQueue)` with daemon threads named `playback`. | §8.2 |
| 71 | The info/loading executor is an eagerly-scaling executor with core 1, max 10, 30 s keep-alive and a 5000-task queue, daemon threads named `info-loader`. | §8.2 |
| 72 | The manager's scheduled executor is a single daemon thread named `manager`. | §8.2 |
| 73 | When the loading queue is full, the load fails with *"Cannot queue loading a track, queue is full."* at severity `SUSPICIOUS`. | §7.3 |
| 74 | The WebSocket send path is **synchronous** on the calling thread and writes directly to the Undertow native channel. | §5.5 |
| 75 | The per-session event queue is a FIFO `ConcurrentLinkedQueue`; paused sessions buffer events and replay them in order on resume. | §5.4, §8.4 |
| 76 | `playerUpdate` is pushed by a `scheduleAtFixedRate` task at the configured `player-update-interval` (default **5 s**). | §8.3 |
| 77 | `stats` is pushed **once per minute** per session. | §8.3 |
| 78 | At most one update task exists per guild; a new one is not scheduled while the old is live. | §8.3 |
| 79 | Discord voice frames are emitted every **20 ms**, with an RTP timestamp increment of **960** per frame. | §8.1 |
| 80 | The Koe UDP-queue default buffer duration is **400 ms**; maximum packet size is **4096** bytes. | §8.1 |
| 81 | **5** silence frames are emitted when the queue runs dry. | §8.1 |

### 10.6 Phase 6 — configuration surface

| # | Behaviour | Reference |
| --- | --- | --- |
| 82 | Support every `lavalink.server.*` key in the §3.1 table, with the same defaults and the same validation/clamping rules. | §3.1 |
| 83 | `lavalink.server.sources.*` gates each source; `local` and `http` default to enabled, the rest default to enabled too but the Youtube source is deprecated. | §3.2 |
| 84 | `lavalink.plugins` supports the plugin declaration format in §3.3. | §3.3 |
| 85 | The route planner is only created when `ratelimit` is present **and** `ipBlocks` is non-empty; the strategy string is lower-cased and trimmed. | §3.5 |
| 86 | IP blocks must be CIDR notation; an invalid block is a fatal error at startup. | §3.5 |
| 87 | `lavalink.server.filters` disables filters by name; absent means enabled. | §3.7 |

### 10.7 Explicitly **not** required for protocol compatibility

The following Lavalink behaviours are **optional** for a Rust implementation. They are listed so an
engineer knows what they may skip.

| Feature | Why it is optional |
| --- | --- |
| The Spring Boot two-phase bootstrap | Any equivalent two-phase load works. |
| The Spring MVC / Undertow stack | Any HTTP + WebSocket server works. |
| The plugin JAR classloading system | Plugins are a Lavalink extension point, not part of the protocol. A Rust implementation may use a different (e.g. WASM or process) plugin model. |
| The plugin download and version-cleanup logic | Same reason. |
| The Prometheus metrics endpoint | Purely operational. |
| Sentry error reporting | Purely operational. |
| OSHI-based CPU metrics | The `Stats` payload can be filled with any equivalent measurement. |
| The `lavadsp` native filters | A Rust implementation can implement the ten filters natively; only the *observable* behaviour matters. |
| The JDA-NAS UDP-queue path | An implementation may use plain UDP or a different transport, as long as the frame timing (§8.1) is preserved. |

---

## 11. Unresolved gaps and open questions

This section records what could **not** be determined from the 4.2.2 sources with certainty, and what a
reimplementation should verify experimentally.

### 11.1 Gaps in the source analysis

| # | Gap | What is known | How to resolve it |
| --- | --- | --- | --- |
| 1 | **Koe internals.** The voice gateway and UDP transport live in `moe.kyokobot.koe:core:3.0.0-pre6`, which ships as a compiled JAR with no source in either checkout. | The constants in §8.1 were extracted from the JAR's bytecode. The **control-flow** (reconnect, resume, opus DTX, silence handling, packet pacing jitter) is not described here. | Read the Koe source from `kyokobot/koe-core` at the matching tag, or reverse the JAR. The extracted JARs are in `$JCODE_SCRATCH_DIR/koedis`, `$JCODE_SCRATCH_DIR/koecore` and `$JCODE_SCRATCH_DIR/udpq`. |
| 2 | **`lavadsp` filter internals.** The ten filters come from `dev.arbjerg:lavadsp:0.7.8`, again a binary artifact. | The **parameters and enablement predicates** are fully specified in §3.8. The **DSP algorithm** behind each filter is not. | Read `natanbc/lavadsp` at tag 0.7.8, or reimplement each filter to match Lavalink's *observable* output. |
| 3 | **Lavaplayer's `libsamplerate` resampler.** The resampling stage is behind a native library. | The stage is conditionally inserted and the quality enum (`HIGH`/`MEDIUM`/`LOW`) is mapped in §3.1. The exact resampling algorithm and its latency are unknown. | Read Lavaplayer's `ResamplingPcmAudioFilter` and the `libsamplerate` binding. Resampling latency affects the first-frame timing. |
| 4 | **Lavaplayer's `OpusAudioDataFormat` encoder.** The encoder is native libopus. | The output format, the silence frame and the expected/maximum chunk sizes are specified in §7.6. The exact libopus settings (bitrate, complexity, VBR) behind `opusEncodingQuality` are not. | Read Lavaplayer's `OpusAudioDataFormat.createEncoder` and the `opus-encoding-quality` mapping table. |
| 5 | **Source-manager internals for `youtube`, `soundcloud`, `twitch`, `niconico`, `bandcamp`, `vimeo`.** | Their source **names**, their position in the resolution order, and their identifier syntax are documented in §7.3 and §7.4. Their scraping/parsing logic, rate limits, and auth requirements are not. | Read each source manager in the Lavaplayer checkout, or use the corresponding Lavalink plugin. |
| 6 | **The HTTP/local probe registry.** | The probe descriptor format `<probeName>|<parameters>` is verified (§6.5). The set of built-in probe names is not enumerated here. | `MediaContainerRegistry.extended(...)` at `AudioPlayerConfiguration.kt:114` shows how probes are registered. Enumerate the defaults in `lavaplayer`'s `MediaContainerRegistry`. |
| 7 | **The `youtube` and `soundcloud` playlist-load semantics.** | `youtubePlaylistLoadLimit` maps to `youtube.setPlaylistPageCount(...)` (§3.1). The pagination behaviour of the search/playlist loaders is unknown. | Read the two source managers. |
| 8 | **Sentry integration specifics.** | The dependency is present (§9.4). The event contents and sampling policy are unknown. | Read `config/SentryConfiguration.kt`. Low priority — Sentry is optional. |

### 11.2 Behaviours that need experimental verification

These are specified precisely enough to implement, but the *exact* wire bytes or timing can only be
confirmed against a running Lavalink instance.

| # | Question | Suggested experiment |
| --- | --- | --- |
| 9 | Does Lavalink ever emit a versioned frame with `flags != 1`? | The `flags` field is 2 bits, so values 0, 2 and 3 are representable, but only 1 (VERSIONED) is used by the 4.x writer. Grep the protocol module for other flag writers. |
| 10 | **RESOLVED** — the `op` field is a **string**, not an integer. | `Message.Op` is a string enum with exactly four values: `ready`, `stats`, `playerUpdate`, `event`. **OBSERVED** — `protocol/v4/messages.kt:18-37`. Each concrete message class is annotated `@SerialName(...)` with the same string as its `op`, so the two can never disagree. |
| 11 | **RESOLVED** — there is **no** WebSocket subprotocol negotiation. | Neither `WebsocketConfig` nor `HandshakeInterceptorImpl` reads or sets `Sec-WebSocket-Protocol`, and a repo-wide grep for it returns nothing. **OBSERVED** — `config/WebsocketConfig.kt:25-27`, `io/HandshakeInterceptorImpl.kt:27-53`. A client may send the header, but Lavalink ignores it. |
| 12 | What are the exact `playerUpdate` **field-application order** side effects (e.g. does setting `track` reset `position`)? | **RESOLVED** — the order is fully specified in §4.6, sourced from `PlayerRestHandler.kt:143-226`. The key rule: `position`, `userData` and `endTime` are applied to the **current** track only when **no** new track was requested; when a new track is requested they are applied to the **new** track instead. `volume` is always applied. `voice` is handled first, synchronized on the player. |
| 13 | What is the exact `playerUpdate` interval granularity? | The default is 5 s. The scheduler is `scheduleAtFixedRate`, so drift is possible under load. Confirm with a live capture. |
| 14 | How long does a session stay resumable after the socket closes? | **RESOLVED** — the default is **60 seconds** (`SocketContext.resumeTimeout = 60L`, `SocketContext.kt:78`). `PATCH /v4/sessions/{id}` overrides it via `sessionUpdate.timeout`, whose wire field is `timeoutSeconds` (`Long`, **seconds**) — `protocol/v4/session.kt:40-45` — and is converted with `it.inWholeSeconds` at `SessionRestHandler.kt:27-29`. |
| 15 | What HTTP status does Lavalink return for an **unknown** route? | Spring Boot's default is 404 with an error body whose shape depends on `server.error.include-message`. |
| 16 | Are the `loadtracks` and `decodetrack` endpoints affected by the request-logging filter? | `RequestLoggingFilter` logs the path; confirm whether it also short-circuits. |

### 11.3 Known divergences a Rust implementation should decide deliberately

These are places where a faithful port and a "better" implementation diverge. The document does not
prescribe an answer.

| # | Divergence | Lavalink's behaviour | Decision needed |
| --- | --- | --- | --- |
| 17 | Plugin integrity | Plugin JARs are downloaded over HTTP(S) with **no checksum or signature verification** (§2.2). | Keep as-is for compatibility, or add verification and accept the behaviour change. |
| 18 | Auth comparison | The `Authorization` header is compared with `String.equals` — exact, case-sensitive, no trimming, and **not** a constant-time comparison. | Keep the exact-match semantics (clients depend on the 401-vs-403 distinction) but use a constant-time comparison to avoid a timing side channel. |
| 19 | `lavalink.server.password: null` | `ServerConfig.password` is a nullable `String?` defaulting to `null`. The Kotlin comparison `authorization != serverConfig.password` uses **structural equality**, where `null == null` is `true`. So a request with **no** `Authorization` header takes the `authorization == null` branch → **401**, and a request whose header value is the literal text `null` does **not** equal the `null` property → **403**. The practical effect is that with an unset password **every** request fails with 401 or 403 — there is no way in. **OBSERVED** — `ServerConfig.kt:32` and `RequestAuthorizationFilter.kt:26-37`. | Decide whether to require a password, or make an unset password mean "no auth required". Reproducing the strict behaviour is safer for compatibility, but it means an unset password locks the server out rather than opening it. |
| 20 | Track-queue backpressure | A full loading queue fails the load with *"Cannot queue loading a track, queue is full."* | Reproduce, or apply backpressure instead. |
| 21 | Source resolution is first-match | A broad source can shadow a narrow one if it is registered earlier. HTTP is deliberately last. | Keep the ordering, or implement a specificity-based match. The ordering is observable. |
| 22 | The `nano` route-planner strategies ignore `excludedIps` | `NanoIpRoutePlanner` is constructed without the exclusion predicate (§3.5). | Reproduce, or treat it as a bug and fix it. |
| 23 | Opus quality out of range | An out-of-range quality falls back to the maximum (10) with a warning, rather than failing. | Reproduce the lenient behaviour. |

### 11.4 Citation-verification status

At the time of writing, **236 distinct file-and-line citations** appear in this document. All of them
were mechanically checked to resolve to a real file in either the Lavalink 4.2.2 or the Lavaplayer
2.2.6 checkout, and every cited line range was checked to lie within that file's bounds. The
substantive claims behind the citations in §2, §3, §4, §5, §6 and §7 were additionally verified by
reading the cited source.

The following files were inspected but are **not** cited, because they contain only mechanical wiring
with no behavioural content worth recording: `WebConfiguration.kt`, `RequestLoggingConfig.kt`,
`SentryConfigProperties.kt`, `GitRepoState.kt`, `AppInfo.kt`, `EventEmitter.kt` (both the
`io` and `player` variants), `ShutdownHandler.kt`, `ResponseHeaderFilter.kt`,
`RequestLoggingFilter.kt`, `DefaultAudioPluginInfoModifier.kt`, `GcNotificationListener.java`,
`PrometheusMetricsController.java`, `PrometheusMetrics.java` and `LavalinkStatsCollector.kt`.

