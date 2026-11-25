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
| Event envelope | flat: `{"op":…,"type":…,"guildId":…,…}` — `op`, `type` and the event's own fields are siblings, with no `data` wrapper | `Message$EmittedEvent` and its per-event serializers in the shipped protocol jar | `nexora-protocol` unit tests |
| Event types | `"TrackStartEvent"`, `"TrackEndEvent"`, `"TrackExceptionEvent"`, `"TrackStuckEvent"`, `"WebSocketClosedEvent"` | `Message$EmittedEvent$Type` in the shipped protocol jar | `compat_protocol_op_codes_are_the_reference_values` |
| Error envelope | `{"timestamp":…,"status":…,"error":…,"trace":…,"message":…,"path":…}`; `timestamp` is epoch milliseconds, not an ISO-8601 string | `Error` and `serialization/TimestampSerializer` in the shipped protocol jar | `nexora-protocol` unit tests |
| Exception severities | `common`, `suspicious`, `fault` — lower case on the wire, although the enum constants are `COMMON`/`SUSPICIOUS`/`FAULT` | `Exception$Severity` in the shipped protocol jar | `nexora-protocol` unit tests |
| Track encoding | Lavaplayer's version-3 track-info frame: a 4-byte big-endian header (flags in the top two bits, payload length in the rest) plus the payload, base64-encoded, with no end marker | `spec:` §6; the reference blob in `docs/research/lavalink-source-architecture.md` §6.4 | `compat_v4_loadtracks_track_shape`, `nexora-protocol` unit tests |
| Load types | `"track"`, `"playlist"`, `"search"`, `"empty"`, `"error"` — lower case on the wire, although the enum constants are `TRACK`/`PLAYLIST`/`SEARCH`/`NONE`/`ERROR` | `ResultStatus` in the shipped protocol jar | `nexora-protocol` unit tests |
| Version negotiation | `4` and `4.x.y` accepted, other generations rejected | `spec:` §1.4 | `nexora-protocol` unit tests |

## REST routes — Lavalink v4.2.2

Reference inventory: `spec:` `docs/research/lavalink-source-architecture.md`
§4.1.

| Route | Method | NEXORA | Status | Notes |
|---|---|---|---|---|
| `/v4/loadtracks` | GET | `loadtracks` | implemented | returns the v4 load result; a search result's `data` is `{ "tracks": [...] }` — `compat_v4_loadtracks_track_shape` |
| `/v4/decodetrack` | GET | `decode_track_route` | implemented | `encodedTrack` takes precedence over `track` — `compat_v4_decodetrack_returns_the_track` |
| `/v4/decodetracks` | POST | `decode_tracks` | implemented | an empty `tracks` list is a 400 — `compat_v4_decodetracks_decodes_a_batch` |
| `/v4/sessions/{id}/players` | GET | `players` | implemented | returns `{ "players": [ Player, ... ] }` — `compat_v4_player_routes` |
| `/v4/sessions/{id}/players/{guildId}` | GET/PATCH/DELETE | `player`, `patch_player`, `delete_player` | implemented | a missing player is a 404; DELETE answers 204 — `compat_v4_player_routes` |
| `/v4/sessions/{id}` | PATCH | `patch_session` | implemented | applies `resuming` and `timeout`; returns the session object — `compat_v4_session_patch_applies_resume_settings` |
| `/v4/info` | GET | `info` | implemented | `plugins` is an object, not a bare array — `compat_v4_info_shape` |
| `/version` | GET | `version` | implemented | plain text, no JSON quotes — `compat_version_endpoint_returns_the_plain_version` |
| `/v4/stats` | GET | `stats` | implemented | the REST payload has no `op`; only the WebSocket `stats` frame carries one — `compat_v4_stats_shape` |
| `/v4/routeplanner/status` | GET | `routeplanner_status` | implemented | 204 when no planner is configured — `compat_v4_routeplanner_status_is_no_content` |
| `/v4/routeplanner/free/address` | POST | `routeplanner_free_address` | implemented | 204; NEXORA has no route planner |
| `/v4/routeplanner/free/all` | POST | `routeplanner_free_all` | implemented | 204; NEXORA has no route planner |
| `/` , `/v3/websocket` | GET | — | not-targeted | Lavalink serves these only to log a deprecation warning |

### NEXORA-only v4 routes

The reference has no route for creating a session: a session is the WebSocket
connection context. NEXORA adds this so a client can obtain a session id
without opening the socket.

| Route | Method | NEXORA | Status | Notes |
|---|---|---|---|---|
| `/v4/sessions` | POST | `create_session` | implemented | NEXORA extension; returns `{ "sessionId": … }` with 201 |

## REST routes — NodeLink extras

| Route | Method | NEXORA | Status | Notes |
|---|---|---|---|---|
| `/v4/loadlyrics` | GET | `load_lyrics` | partial | answers an empty payload; NEXORA has no lyrics source |
| `/v4/loadchapters` | GET | `load_chapters` | partial | answers `[]`; NEXORA has no chapters source |
| `/v4/connection` | GET | `connection` | partial | reports the reference's disabled payload; NEXORA has no voice gateway |
| `/v4/trackstream/{encodedTrack}` | GET | `trackstream` | partial | transcodes a local file to Opus; other source types are a 422 |
| `/v4/loadstream` | GET | — | missing | M8; raw PCM streaming, gated by config |
| `/v4/hls/*` | GET | — | not-targeted | HLS serving is a NodeLink feature, not Lavalink's |
| `/v4/profiler*` | GET | — | not-targeted | NodeLink profiling endpoints |
| `/v4/sources/health` | GET | — | missing | M8; source health snapshot |
| `/v4/workers` | GET | — | not-targeted | NodeLink's two-tier worker model |
| `/v4/youtube/config`, `/v4/youtube/oauth` | GET | — | not-targeted | YouTube provider surface, out of scope |

## WebSocket

| Endpoint | NEXORA | Status | Notes |
|---|---|---|---|
| `/v4/websocket` | `websocket` | implemented | Lavalink's single event socket; the `ready` frame is flat and comes first — `compat_v4_websocket_ready_frame`, `compat_v4_websocket_handshake_refuses_a_bad_request`, `compat_v4_websocket_handshake_refuses_a_wrong_password` |
| `/ws/v1/sessions/{id}` | `websocket_events` | partial | NEXORA's own per-session socket; the v4 shape is not served here |
| `/v4/websocket/voice` | — | not-targeted | NodeLink voice socket |
| `/v4/websocket/youtube/live` | — | not-targeted | NodeLink live-metadata socket |

## Events

Reference: NodeLink `src/constants.ts` `GatewayEvents` (28 entries), read from
the analysed tree.

Every frame is flat: `op`, the `type` discriminator, `guildId` and the
event's own fields are siblings, with no `data` wrapper (bytecode evidence on
`Message$EmittedEvent` in the shipped `protocol-jvm-4.2.2.jar`).

| Event | Lavalink | NodeLink | NEXORA | Status |
|---|---|---|---|---|
| `TrackStartEvent` | yes | yes | `V4EventMessage::TrackStart` on `/v4/websocket` | implemented — `compat_v4_track_start_event_carries_the_track` |
| `TrackEndEvent` | yes | yes | `V4EventMessage::TrackEnd` on `/v4/websocket` | implemented — `compat_v4_track_end_event_carries_the_reason` |
| `TrackExceptionEvent` | yes | yes | `V4EventMessage::TrackException` on `/v4/websocket` | implemented — `compat_v4_track_exception_carries_the_cause` |
| `playerUpdate` | yes | yes | `V4PlayerUpdateFrame` on `/v4/websocket` | implemented — `compat_v4_player_update_state_shape` |
| `WebSocketClosedEvent` | yes | yes | `V4EventMessage::WebSocketClosed` on `/v4/websocket` | implemented |
| `TrackStuckEvent` | no | yes | `V4EventMessage::TrackStuck` on `/v4/websocket` | partial | the type and shape are served; NEXORA's pipeline is not a live stream, so it cannot stall the way the reference's does |
| `SeekEvent` | no | yes | `V4EventMessage::Seek` on `/v4/websocket` | implemented — `compat_v4_player_events_report_the_change` |
| `PauseEvent` | no | yes | `V4EventMessage::Pause` on `/v4/websocket` | implemented — `compat_v4_player_events_report_the_change` |
| `VolumeChangedEvent` | no | yes | `V4EventMessage::VolumeChanged` on `/v4/websocket` | implemented — `compat_v4_player_events_report_the_change` |
| `FiltersChangedEvent` | no | yes | `V4EventMessage::FiltersChanged` on `/v4/websocket` | implemented — `compat_v4_player_events_report_the_change` |
| `PlayerCreatedEvent` | no | yes | `V4EventMessage::PlayerCreated` on `/v4/websocket` | implemented — `compat_v4_player_events_report_the_change` |
| `PlayerDestroyedEvent` | no | yes | `V4EventMessage::PlayerDestroyed` on `/v4/websocket` | implemented — `compat_v4_player_events_report_the_change` |
| `VoiceStateUpdate` | yes | yes | `V4EventMessage::VoiceStateUpdate` echoed on `/v4/websocket` | implemented — `compat_v4_voice_state_update_echoes_back` |
| `PlayerReconnectingEvent`, `PlayerConnectedEvent`, `ConnectionStatusEvent` | no | yes | — | missing (M5) — they need the voice gateway |
| `MixStartedEvent`, `MixEndedEvent` | no | yes | — | not-targeted (mixing is not in scope) |
| `SponsorBlockSegmentsLoadedEvent`, `SponsorBlockSegmentSkippedEvent` | no | yes | — | not-targeted (SponsorBlock is a plugin capability) |
| `StreamMetadataEvent`, `SourceHealthSnapshotEvent`, `SourceHealthChangeEvent` | no | yes | — | missing (M8) |
| `EternalBoxInfoEvent`, `EternalBoxJumpEvent` | no | yes | — | not-targeted (EternalBox specific) |

### Event ordering

The reference emits, for one track: the `TrackStartEvent`, then a
`playerUpdate` immediately (the update task's initial delay is zero), then a
`playerUpdate` every five seconds, then the `TrackEndEvent`. The update task
is cancelled when the track ends, so no `playerUpdate` follows the
`TrackEndEvent` (bytecode evidence on `LavalinkPlayer.onTrackStart`,
`onTrackEnd` and `SocketServer$Companion.sendPlayerUpdate` in the shipped
`Lavalink.jar`).

NEXORA reproduces that order — `compat_v4_event_order_track_start_update_end`
asserts it — and skips the update entirely while the player is paused, the
way the reference does —
`compat_v4_paused_player_sends_no_player_update`.

The v4 protocol has **no** queue surface: the queue lives in the client, and
the `TrackEndEvent` reason tells it whether to advance. NEXORA keeps its own
queue in the native session manager and publishes the reason as well, so a
client that keeps a queue can advance its own.

## Track end reasons

Reference: NodeLink `src/constants.ts` `EndReasons`.

| Reason | Lavalink | NodeLink | NEXORA | Status |
|---|---|---|---|---|
| `stopped` | yes | yes | `V4TrackEndReason::Stopped` | implemented |
| `finished` | yes | yes | `V4TrackEndReason::Finished` | implemented |
| `loadFailed` | yes | yes | `V4TrackEndReason::LoadFailed` | implemented |
| `replaced` | yes | yes | `V4TrackEndReason::Replaced` | implemented |
| `cleanup` | no | yes | `V4TrackEndReason::Cleanup` | partial | typed, never emitted — it belongs to the player's destruction |
| `gapless` | no | yes | `V4TrackEndReason::Gapless` | partial | typed, never emitted — gapless is a separate capability |
| `crossfading` | no | yes | `V4TrackEndReason::Crossfading` | partial | typed, never emitted — crossfade is not in scope |

## Filters

Reference: Lavalink's ten built-in filters are the extension list its
`InfoRestHandler` advertises — `volume`, `equalizer`, `karaoke`, `timescale`,
`tremolo`, `vibrato`, `distortion`, `rotation`, `channelMix`, `lowPass`
(craft-anything `java_class_show` on `lavalink.server.info.InfoRestHandler` in
the shipped `Lavalink.jar`); NodeLink ships 21 filter classes plus a mixer
(`src/playback/filters/`, `src/playback/processing/`).

| Filter | Lavalink | NodeLink | NEXORA | Status |
|---|---|---|---|---|
| `volume`, `equalizer`, `karaoke`, `timescale` | yes | yes | typed, not applied | partial (M4) |
| `tremolo`, `vibrato`, `distortion`, `rotation` | yes | yes | typed, not applied | partial (M4) |
| `channelMix`, `lowPass` | yes | yes | typed, not applied | partial (M4) |
| `highpass`, `compressor`, `echo`, `phaser` | no | yes | typed, not applied | partial (M4, NodeLink extra) |
| `chorus`, `flanger`, `reverb`, `spatial`, `phonograph`, `tesseract` | no | yes | — | not-targeted |
| JS script filters | yes (Rhino engine bundled) | no | — | missing (M4); needs a JS engine in Rust |
| Audio mixer (TTS/SFX layering) | no | yes | — | not-targeted |

The filter chain is typed in `crates/nexora-protocol/src/v4/filters.rs` with the
reference's parameter names and defaults, and `PATCH .../players/{guildId}`
stores it on the player. No filter is applied to the audio stream yet, so
`/v4/info` reports an empty `filters` list — the reference advertises only the
filters it actually applies.

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
| Lavalink v4 routes | 13 | 0 | 0 | 1 |
| NEXORA-only v4 routes | 1 | 0 | 0 | 0 |
| NodeLink extra routes | 0 | 4 | 2 | 4 |
| WebSocket endpoints | 1 | 1 | 0 | 2 |
| Events | 11 | 1 | 2 | 3 |
| Track end reasons | 4 | 3 | 0 | 0 |
| Filters | 0 | 4 | 1 | 2 |
| Sources | 3 | 0 | 0 | 2 |

No row may be called compatible on the strength of this table: a row is
`implemented` only when a test in `tests/compat` exercises it against the
reference behaviour.
