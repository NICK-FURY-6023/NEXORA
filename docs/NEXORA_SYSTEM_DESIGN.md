# NEXORA — Independent Rust Audio System Design

**Goal:** A complete, lightweight, from-scratch Rust audio engine + service platform (a Lavalink alternative) with **zero third-party runtime dependency** — no JVM, no Spring Boot, no Lavaplayer service, no Lavalink wrapper. Native Rust codec crates are acceptable as *libraries* (not an external runtime).

**Evidence base:** `docs/LAVALINK_DEEP_DIVE.md`, `docs/research/lavalink-rea-static-analysis.md`, cloned `lavalink-devs/Lavalink@4.2.2` source, official `lavalink.dev` v4 REST/WS/plugin docs. All structural claims carry OBSERVED / INFERRED labels.

**Mandatory skills used:** reverse-engineer-anything (REA), rust-anything, ecc-guide, using-superpowers (global memory `mem_1795359430838_11883272316274892010`).

**ADRs referenced:** ADR-015 (Rust core, no JVM), ADR-016 (native `/api/v1` + `/v4` compat layer), ADR-017 (in-memory sessions), ADR-020 (bounded queues).

---

## 1. Design constraints (non-negotiable)

| Constraint | Rationale |
|------------|-----------|
| Zero third-party runtime dependency | User mandate: own system, not Lavalink's deps |
| From-scratch Rust | ADR-015 |
| Native `/api/v1` + optional `/v4` compat | ADR-016 |
| In-memory sessions | ADR-017 |
| Bounded queues | ADR-020 |
| OBSERVED/INFERRED evidence standard | Project policy |

**Out of scope for v1 (explicit gaps):** YouTube/SoundCloud/HTTP source resolution (Lavalink's source managers live in Lavaplayer, not the server — NEXORA core does not bundle them), plugin marketplace/maven downloads, route-planner IP rotation, Sentry error reporting, Prometheus metrics (can be added later), voice resume across node restarts (session resume in-memory only).

---

## 2. High-level architecture

```
┌─────────────────┐      REST (JSON)       ┌──────────────────────┐
│   Bot client    │ ◄─────────────────────► │  HTTP server (axum)  │
│  (Discord bot)  │      /api/v1, /v4      │   sessions/players   │
└─────────────────┘                        └──────────┬───────────┘
                                                     │ WS (frames)
                                                     │ /api/v1/ws
                                                     ▼
                                            ┌──────────────────────┐
                                            │   Voice gateway      │
                                            │   (UDP, Opus, NaCl)  │
                                            └──────────┬───────────┘
                                                       │ Discord voice
                                                       ▼
                                                  Discord voice server
```

**Layers (Rust crates):**

| Crate | Responsibility | Replaces Lavalink |
|-------|---------------|-------------------|
| `nexora-core` | Session, player, track container, filter DSP, bounded queue, audio pipeline orchestrator | Lavaplayer + Lavalink server logic |
| `nexora-server` | HTTP (`/api/v1`, `/v4`), WebSocket, auth, stats, route-planner stubs | Spring Boot webmvc/websocket |
| `nexora-voice` | Discord voice-gateway client: UDP, secret-box encryption, Opus packetization, 20 ms frame pacing | koe + netty |
| `nexora-codecs` | Opus encode/decode, PCM resample, optional MP3/FLAC/Vorbis decode (feature-gated) | lavaplayer-natives + lavadsp |
| `nexora-session` (exists) | In-memory session store (ADR-017) | `SocketServer.sessions` |
| `nexora-source-framework` (exists) | Trait for source resolution (stub: URL passthrough; sources opt-in) | `AudioSourceManager` |
| `nexora-plugin-api` (exists) | Trait-based plugin ABI (`libloading`/WASM), no classpath | `PluginManager` + `plugin-api` |
| `nexora-cluster` (exists) | Node discovery/hand-off stub | (none in Lavalink v4 core) |

---

## 3. Data model

### 3.1 Track (NEXORA container — own format, not Lavaplayer byte-compatible)
```rust
pub struct Track {
    pub encoded: String,        // Base64 of NEXORA binary container (opaque to client)
    pub info: TrackInfo,
    pub plugin_info: serde_json::Value,   // {}
    pub user_data: serde_json::Value,     // {}
}

pub struct TrackInfo {
    pub identifier: String,
    pub is_seekable: bool,
    pub author: String,
    pub length_ms: u64,
    pub is_stream: bool,
    pub position_ms: u64,
    pub title: String,
    pub uri: Option<String>,
    pub source_name: String,
    pub artwork_url: Option<String>,
    pub isrc: Option<String>,
}
```
**OBSERVED:** mirrors Lavalink `Track`/`TrackInfo` exactly (11 fields) for client compatibility. **INFERRED:** `encoded` is NEXORA's own Base64 container (identifier + info + opaque source payload + position). Not byte-compatible with Lavaplayer `MessageOutput` — documented gap.

### 3.2 Session & player
```rust
pub struct Session {
    pub id: SessionId,              // 16-char alnum (OBSERVED Lavalink format)
    pub players: DashMap<GuildId, Player>,
    pub resumable: bool,
    pub resume_timeout_secs: u64,   // default 60 (OBSERVED)
    pub voice_server_info: Option<VoiceServerInfo>, // token/endpoint/sessionId/channelId
}

pub struct Player {
    pub guild_id: GuildId,
    pub track: Option<Track>,
    pub volume: u16,                // 0..=1000 (OBSERVED)
    pub paused: bool,
    pub state: PlayerState,         // {time, position, connected, ping}
    pub filters: FilterChain,
    pub audio_player: AudioPlayer,  // NEXORA own (plays decoded PCM → Opus frames)
}
```

**OBSERVED:** Lavalink `Player`/`PlayerState`/`VoiceState` shapes mirrored. **INFERRED:** `AudioPlayer` is NEXORA's own decode→filter→resample→encode loop (replaces Lavaplayer `AudioPlayer`).

### 3.3 Filter chain (fixed order — OBSERVED from Lavalink `FilterChain.buildList()`)
1. volume
2. equalizer (15 bands: 25, 40, 63, 100, 160, 250, 400, 630, 1000, 1600, 2500, 4000, 6300, 10000, 16000 Hz; gain −0.25…1.0)
3. karaoke
4. timescale (speed, pitch, rate)
5. tremolo (frequency, depth)
6. vibrato (frequency, depth)
7. distortion (sin/cos/tan offset & scale, offset, scale)
8. rotation (rotationHz)
9. channelMix (leftToLeft, leftToRight, rightToLeft, rightToRight)
10. lowPass (smoothing)
11. plugin filters (last)

**NEXORA:** `FilterChain` trait + built-in impls in `nexora-core`; plugin filters via `nexora-plugin-api` `AudioFilterExtension` trait.

---

## 4. Audio pipeline (replaces Lavaplayer — NEXORA own)

```
Source resolver (URL/identifier)  ►  Demux/container parse  ►  Decode to PCM (f32)
                                              │
                                              ▼
                                        FilterChain (ordered DSP)
                                              │
                                              ▼
                                        Resample to 48kHz stereo
                                              │
                                              ▼
                                        Opus encode (20 ms frames)
                                              │
                                              ▼
                                Voice gateway UDP → Discord
```

**Stages (crates):**
1. `nexora-source-framework::SourceResolver` trait — `resolve(identifier) -> Result<Track, SourceError>`. v1 ships a **passthrough** resolver (identifier treated as URL/file); YouTube/SoundCloud are opt-in source crates, not in core (gap declared).
2. `nexora-codecs` — decode MP3/FLAC/Ogg/Opus to f32 PCM (feature-gated crates); resample (speexdsp/rubato); Opus encode (opus crate).
3. `nexora-core::FilterChain` — DSP on f32 PCM frames.
4. `nexora-voice` — packetize Opus into Discord voice UDP packets (secret-box nonce/encryption, sequence/timestamp), 20 ms frame pacing.

**INFERRED:** Exact Discord voice packet format (nonce construction, encryption overhead, RTP header) is NOT reverse-engineered from koe — declared gap, to be implemented against Discord API docs.

---

## 5. API surface (REST + WebSocket)

### 5.1 REST (`/api/v1` native; `/v4` compat alias per ADR-016)
All require `Authorization: <password>` header.

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/v1/loadtracks?identifier=` | GET | resolve → `LoadResult` (track|playlist|search|empty|error) |
| `/api/v1/decodetrack?encodedTrack=` | GET | decode one track |
| `/api/v1/decodetracks` | POST | decode array |
| `/api/v1/sessions/{sessionId}/players` | GET | list players |
| `/api/v1/sessions/{sessionId}/players/{guildId}` | GET | get player |
| `/api/v1/sessions/{sessionId}/players/{guildId}?noReplace=` | PATCH | update/create player |
| `/api/v1/sessions/{sessionId}/players/{guildId}` | DELETE | destroy player |
| `/api/v1/sessions/{sessionId}` | PATCH | update session (resuming, timeout) |
| `/api/v1/info` | GET | version/build/sourceManagers/filters/plugins |
| `/api/v1/stats` | GET | players, playingPlayers, uptime, memory, cpu |
| `/api/v1/version` | GET | plain version |
| `/v4/...` | same | `/v4` alias → `/api/v1` (ADR-016) |

### 5.2 WebSocket (`/api/v1/websocket`)
Headers: `Authorization`, `User-Id`, `Client-Name`, optional `Session-Id`. Server→client ops: `ready`, `playerUpdate`, `stats`, `event` (TrackStart/End/Exception/Stuck/WebSocketClosed). Client→server text: **not supported** (mirrors Lavalink v4 — OBSERVED `handleTextMessage` warning).

**playerUpdate interval:** default 5s (OBSERVED Lavalink `playerUpdateInterval`).

---

## 6. Concurrency & queue model (ADR-020)

- **Per-session:** one `Session` struct; players stored in `DashMap<GuildId, Player>` (lock-free reads, bounded writes).
- **Per-player:** single playback task (tokio spawn) driving decode→filter→encode loop; bounded `mpsc` queue of Opus frames (ADR-020 bounded queues) with backpressure.
- **playerUpdate scheduler:** tokio `interval` at `playerUpdateInterval` (default 5s), broadcasts `playerUpdate` op on the session WS.
- **Stats:** background task every 60s computing players/playingPlayers/uptime/memory/cpu/frameStats.
- **Session resume:** in-memory `resumable_sessions` map; resume within `resume_timeout_secs` (default 60s); else shutdown (ADR-017 in-memory only).

**INFERRED:** Frame pacing target = 3000 frames/minute per player (1 per 20 ms) — Lavalink `frameStats.deficit` semantics mirrored; exact deficit math not verified from source.

---

## 7. Plugin model (replaces classpath `PluginManager`)

**NEXORA approach (INFERRED, no JVM classpath):**
- `nexora-plugin-api` defines traits: `AudioSourceManager`, `MediaContainerProbe`, `RestInterceptor`, `AudioPluginInfoModifier`, `AudioFilterExtension`.
- Plugins are loaded via `libloading` (native `.so`/`.dll`/`.dylib`) or a WASM ABI — plugin declares `#[no_mangle] extern "C" fn nexora_plugin_manifest() -> PluginManifest { name, version, exports }`.
- `PluginManager` scans `plugins_dir/*.so` (or `.wasm`), loads manifest, registers trait objects into the server context.
- No maven downloads, no `URLClassLoader`, no `lavalink-plugins/*.properties`. Plugin repo is opt-in configuration, not runtime.

---

## 8. Configuration (replaces `application.yml`)

| Key | Type | Default | Source |
|-----|------|---------|--------|
| `server.password` | String | required | auth |
| `server.bind` | host:port | `0.0.0.0:2333` | listener |
| `session.resume_timeout_secs` | u64 | 60 | resume window |
| `player.update_interval_secs` | u64 | 5 | playerUpdate |
| `audio.opus_encoding_quality` | u8 | default | Opus quality |
| `audio.frame_buffer_duration_ms` | u32 | — | bounded queue |
| `audio.non_allocating_frame_buffer` | bool | false | ADR-020 |
| `filters.*` | map | empty | filter enable flags |
| `plugins.dir` | path | `./plugins` | plugin dir |
| `plugins.[*].dependency` | str | required | `name:version` |
| `logs.level` | str | `info` | tracing |

**OBSERVED:** mirrors Lavalink `ServerConfig` keys where they exist; NEXORA uses its own simple config (no Spring `application.yml` complexity — INFERRED simplification).

---

## 9. What NEXORA keeps vs drops (from Lavalink)

| Keep (semantics mirrored) | Drop / replace |
|---------------------------|----------------|
| Session → players → guild scoping | JVM / Spring Boot |
| REST + WS split (REST commands, WS events) | Lavaplayer (NEXORA own audio pipeline) |
| Ordered filter chain (10 built-in + plugin) | koe / netty (NEXORA own UDP voice client) |
| Opaque `encoded` track blob (own NEXORA format) | Maven `URLClassLoader` plugin loading (`libloading`/WASM) |
| Password auth | `application.yml` (NEXORA own config) |
| In-memory sessions (ADR-017), bounded queues (ADR-020) | — |
| Track Info 11 fields, volume 0–1000, playerUpdate shape | — |

---

## 10. Build order (crates, dependencies)

```
nexora-codecs       <- opus + (optional) symphonia/rodio decode crates
nexora-session      <- std + dashmap (exists)
nexora-source-framework <- std (exists, trait only)
nexora-plugin-api   <- std + serde_json (exists, trait only)
nexora-core         <- nexora-session, nexora-source-framework, nexora-codecs, tokio, dashmap
nexora-voice        <- tokio, udp, sodiumoxide/libsodium, opus
nexora-server       <- axum, tokio-tungstenite, serde, serde_json, nexora-core, nexora-voice
nexora-cluster      <- std (exists, stub)
```

**Dependency discipline (rust-anything):** every new crate added must be justified against the zero-third-party-runtime rule; codecs are libraries, not runtime services. `Cargo.toml` must be audited before merge.

---

## 11. Gaps / unknowns (honest reporting)

1. **Lavaplayer binary track format** not reverse-engineered — NEXORA defines its own `encoded` container (declared, documented).
2. **koe / Discord voice gateway internals** not inspected — UDP + Opus + NaCl packet format to be implemented against Discord docs (declared).
3. **Source managers (YouTube/SoundCloud/HTTP/local)** not in NEXORA core v1 — passthrough resolver only; sources opt-in crates (declared).
4. **Runtime thread-pool sizes, frame pacing, deficit math** not verified from source — NEXORA chooses its own defaults (declared).
5. **Plugin ABI stability** not yet designed — `libloading`/WASM contract TBD (declared).
6. **Static analysis cannot prove runtime behaviour** — all concurrency/timing claims INFERRED.

---

## 12. Immediate next steps

1. Verify swarm source-worker output (`docs/research/lavalink-source-architecture.md` — not yet materialized; worker output pending).
2. Finish `rust-anything` CLI install (background task `180657tkw3` still compiling — required for porting discipline).
3. Implement `nexora-core` track container + filter chain (mirroring sections 3.1–3.3, OBSERVED order).
4. Implement `nexora-server` `/api/v1` REST + `/v4` alias + WebSocket (section 5).
5. Implement `nexora-voice` UDP voice gateway (section 4, declared gap).
6. Write `docs/NEXORA_ARCHITECTURE.md` update + ADR for the new design; add unit tests for filter ordering and track encode/decode round-trip.
