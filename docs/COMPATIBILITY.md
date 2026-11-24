# NEXORA compatibility matrix

Status: draft, 2026-11-24. Target baseline: Lavalink v4.2.2 plus the
NodeLink extensions worth having.

Each row states what the reference does, what NEXORA does today, the status,
and the evidence. Status values: `implemented` (shipped and tested),
`partial` (shipped, shape differs), `missing` (not shipped), `not-targeted`
(will not be reproduced, with the reason).

Evidence ids: `rea: ev_…` are REA records, `ev_…` are craft-anything records,
`spec:` points at the line-cited behavioural spec in
`docs/research/lavalink-source-architecture.md`.

## Reference identity

| Fact | Value | Evidence |
|---|---|---|
| Lavalink release | `Implementation-Version: 4.2.2`, `Implementation-Title: Lavalink-Server` | `ev_3cf63a973acdb1c36445797c6444946984a24f8f792fb7f13865c6dfc13471e5` |
| Lavalink packaging | Spring Boot fat jar, `JarLauncher`, 163 classes, 133 nested jars, Java 17 | `ev_3cf63a973acdb1c36445797c6444946984a24f8f792fb7f13865c6dfc13471e5` |
| Lavalink audio engine | `lavaplayer-2.2.6.jar` + `lavaplayer-natives-2.2.6.jar` (27 MB native codecs for linux/win/darwin) | `ev_3cf63a973acdb1c36445797c6444946984a24f8f792fb7f13865c6dfc13471e5` |
| Lavalink filters | `lavadsp-0.7.8.jar`; `lavalink.server.player.filters` holds 15 classes | `ev_3cf63a973acdb1c36445797c6444946984a24f8f792fb7f13865c6dfc13471e5` |
| Lavalink JS filters | `rhino-1.7.14.jar` + `rhino-engine-1.7.14.jar` — a JavaScript engine is bundled | `ev_3cf63a973acdb1c36445797c6444946984a24f8f792fb7f13865c6dfc13471e5` |
| Lavalink DAVE | `tink-1.14.1.jar` bundled — DAVE's crypto library | `ev_3cf63a973acdb1c36445797c6444946984a24f8f792fb7f13865c6dfc13471e5` |
| Lavalink voice | `udpqueue-*` native jars + `natives-*` per platform | `ev_3cf63a973acdb1c36445797c6444946984a24f8f792fb7f13865c6dfc13471e5` |
| Lavalink plugin API | `plugin-api-4.2.2.jar` — a separate artifact plugins compile against | `ev_3cf63a973acdb1c36445797c6444946984a24f8f792fb7f13865c6dfc13471e5` |
| NodeLink size | 686 files (337 TypeScript, 337 JavaScript), 185,577 TS LOC | craft-anything `app_source_import`, `E:\nodelink-src.dtcD` |
| NodeLink analysis | 29 compatibility files analysed, 427 findings, 19,852 data-flow relations | `rea: ev_c5adb52346647930108c05a95bfdbd85b8f096e4c424bcd041320c47272b5370` |

## Protocol core (M0)

The v4 wire types NEXORA owns are typed in `crates/nexora-protocol/src/v4/` and
proved by `services/nexora-server/tests/compat/`.

| Piece | Wire contract | Evidence | Proof |
|---|---|---|---|
| Op codes | `"ready"`, `"stats"`, `"playerUpdate"`, `"event"` | `Message$Op` in the shipped `protocol-jvm-4.2.2.jar` | `compat_protocol_op_codes_are_the_reference_values` |
| Event envelope | `{"op":…,"event":…,"data":…}`, `event` omitted when absent | `Message$EmittedEvent` | `nexora-protocol` unit tests |
| Error envelope | `{"timestamp":…,"error":{"code","message","severity","cause"}}`, severities `COMMON`/`SUSPICIOUS`/`FAULT` | `Exception$Severity` in the shipped protocol jar | `nexora-protocol` unit tests |
| Track encoding | base64 of `{track_id, source_id, title, duration_ms, artist}`; decodes to the v4 `info` shape | `docs/LAVALINK_DEEP_DIVE.md` §3 (semantic, not byte, compatibility) | `compat_v4_loadtracks_track_shape` |
| Version negotiation | `4` and `4.x.y` accepted, other generations rejected | `spec:` §1.4 | `nexora-protocol` unit tests |

## REST routes — Lavalink v4.2.2

Reference inventory: `spec:` `docs/research/lavalink-source-architecture.md`
§4.1.

| Route | Method | NEXORA | Status | Notes |
|---|---|---|---|---|
| `/v4/loadtracks` | GET | `lavalink_loadtracks` | implemented | returns the v4 track shape |
| `/v4/decodetrack` | GET | — | missing | M7 |
| `/v4/decodetracks` | POST | — | missing | M7 |
| `/v4/sessions/{id}/players` | GET | — | missing | M7; NEXORA exposes `/v4/sessions/{id}/player` (singular) instead |
| `/v4/sessions/{id}/players/{guildId}` | GET/PATCH/DELETE | — | missing | M7 |
| `/v4/sessions/{id}` | PATCH | — | missing | M2 |
| `/v4/info` | GET | `lavalink_info` | implemented | |
| `/version` | GET | — | missing | M1; NodeLink serves it as `/v4/version` |
| `/v4/stats` | GET | `lavalink_stats` | implemented | |
| `/v4/routeplanner/status` | GET | `lavalink_routeplanner_status` | implemented | |
| `/v4/routeplanner/free/address` | POST | — | missing | M7 |
| `/v4/routeplanner/free/all` | POST | — | missing | M7 |
| `/` , `/v3/websocket` | GET | — | not-targeted | Lavalink serves these only to log a deprecation warning |

## REST routes — NodeLink extras

| Route | Method | NEXORA | Status | Notes |
|---|---|---|---|---|
| `/v4/loadlyrics` | GET | — | missing | M8; `&lang=` optional |
| `/v4/loadchapters` | GET | — | missing | M8 |
| `/v4/connection` | GET | — | missing | M8; NodeLink-exclusive, carries speed/bytes metrics |
| `/v4/trackstream` | GET | — | missing | M8; gated by config, returns direct formats |
| `/v4/loadstream` | GET | — | missing | M8; raw PCM streaming, gated by config |
| `/v4/hls/*` | GET | — | not-targeted | HLS serving is a NodeLink feature, not Lavalink's |
| `/v4/profiler*` | GET | — | not-targeted | NodeLink profiling endpoints |
| `/v4/sources/health` | GET | — | missing | M8; source health snapshot |
| `/v4/workers` | GET | — | not-targeted | NodeLink's two-tier worker model |
| `/v4/youtube/config`, `/v4/youtube/oauth` | GET | — | not-targeted | YouTube provider surface, out of scope |

## WebSocket

| Endpoint | NEXORA | Status | Notes |
|---|---|---|---|
| `/v4/websocket` | — | missing | M1; Lavalink's single event socket |
| `/ws/v1/sessions/{id}` | `websocket_events` | partial | NEXORA's own per-session socket; the v4 shape is not served yet |
| `/v4/websocket/voice` | — | not-targeted | NodeLink voice socket |
| `/v4/websocket/youtube/live` | — | not-targeted | NodeLink live-metadata socket |

## Events

Reference: NodeLink `src/constants.ts` `GatewayEvents` (28 entries), read from
the analysed tree.

| Event | Lavalink | NodeLink | NEXORA | Status |
|---|---|---|---|---|
| `TrackStartEvent` | yes | yes | `track_started` (native event, different channel) | partial |
| `TrackEndEvent` | yes | yes | `track_ended` | partial |
| `TrackExceptionEvent` | yes | yes | `track_failed` | partial |
| `playerUpdate` | yes | yes | — | missing (M3) |
| `WebSocketClosedEvent` | yes | yes | — | missing (M3) |
| `TrackStuckEvent` | no | yes | — | missing (M3) |
| `SeekEvent`, `PauseEvent`, `VolumeChangedEvent`, `FiltersChangedEvent` | no | yes | — | missing (M3) |
| `PlayerCreatedEvent`, `PlayerDestroyedEvent`, `PlayerReconnectingEvent`, `PlayerConnectedEvent`, `ConnectionStatusEvent` | no | yes | — | missing (M3) |
| `MixStartedEvent`, `MixEndedEvent` | no | yes | — | not-targeted (mixing is not in scope) |
| `SponsorBlockSegmentsLoadedEvent`, `SponsorBlockSegmentSkippedEvent` | no | yes | — | not-targeted (SponsorBlock is a plugin capability) |
| `StreamMetadataEvent`, `SourceHealthSnapshotEvent`, `SourceHealthChangeEvent` | no | yes | — | missing (M8) |
| `EternalBoxInfoEvent`, `EternalBoxJumpEvent` | no | yes | — | not-targeted (EternalBox specific) |

## Track end reasons

Reference: NodeLink `src/constants.ts` `EndReasons`.

| Reason | Lavalink | NodeLink | NEXORA | Status |
|---|---|---|---|---|
| `stopped` | yes | yes | `{"reason":"finished"}` only | partial (M3) |
| `finished` | yes | yes | yes | implemented |
| `loadFailed` | yes | yes | `{"reason": reason}` | partial |
| `replaced` | yes | yes | — | missing (M3) |
| `cleanup` | no | yes | — | not-targeted |
| `gapless` | no | yes | — | missing (M8) |
| `crossfading` | no | yes | — | not-targeted (crossfade is not in scope) |

## Filters

Reference: Lavalink's ten built-in filters are the extension list its
`InfoRestHandler` advertises — `volume`, `equalizer`, `karaoke`, `timescale`,
`tremolo`, `vibrato`, `distortion`, `rotation`, `channelMix`, `lowPass`
(craft-anything `java_class_show` on `lavalink.server.info.InfoRestHandler` in
the shipped `Lavalink.jar`); NodeLink ships 21 filter classes plus a mixer
(`src/playback/filters/`, `src/playback/processing/`).

| Filter | Lavalink | NodeLink | NEXORA | Status |
|---|---|---|---|---|
| `volume`, `equalizer`, `karaoke`, `timescale` | yes | yes | — | missing (M4) |
| `tremolo`, `vibrato`, `distortion`, `rotation` | yes | yes | — | missing (M4) |
| `channelMix`, `lowPass` | yes | yes | — | missing (M4) |
| `highpass`, `compressor`, `echo`, `phaser` | no | yes | — | missing (M4, NodeLink extra) |
| `chorus`, `flanger`, `reverb`, `spatial`, `phonograph`, `tesseract` | no | yes | — | not-targeted |
| JS script filters | yes (Rhino engine bundled) | no | — | missing (M4); needs a JS engine in Rust |
| Audio mixer (TTS/SFX layering) | no | yes | — | not-targeted |

## Sources

| Surface | NEXORA | Status |
|---|---|---|
| Local files | `LocalMediaAdapter` | implemented |
| HTTP streams | `AuthorizedHttpAdapter` | implemented |
| Radio | `PublicRadioAdapter` | implemented |
| Spotify, Apple Music, Tidal, Deezer, YouTube and the rest of NodeLink's managers | — | not-targeted (commercial providers are a documented NEXORA non-goal) |
| Lavalink's Java plugin ecosystem | — | not-targeted (a pure-Rust server cannot load a Java plugin; the WASM runtime is the substitute, M10) |

## Summary

Counts are matrix rows. A grouped row lists several items — the Filters table
covers 21 filters in 7 rows, and the Events table covers 28 events in 12 rows.

| Surface | implemented | partial | missing | not-targeted |
|---|---|---|---|---|
| Lavalink v4 routes | 4 | 0 | 8 | 1 |
| NodeLink extra routes | 0 | 0 | 6 | 4 |
| WebSocket endpoints | 0 | 1 | 1 | 2 |
| Events | 0 | 3 | 6 | 3 |
| Track end reasons | 1 | 2 | 2 | 2 |
| Filters | 0 | 0 | 5 | 2 |
| Sources | 3 | 0 | 0 | 2 |

No row may be called compatible on the strength of this table: a row is
`implemented` only when a test in `tests/compat` exercises it against the
reference behaviour.
