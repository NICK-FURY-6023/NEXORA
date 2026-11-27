# NEXORA Protocol

REST + WebSocket endpoint specification for NEXORA.

This document describes the implemented wire contracts. It is the authoritative reference for the native `/api/v1` contract, the native audio WebSocket, the event WebSocket, and the Lavalink-v4-compatible adapter, which is served under the `/nexora/*` prefix (there are no `/v4/*` routes). Machine-readable contracts (OpenAPI / protobuf) live in `crates/nexora-protocol` and `proto/nexora.proto`.

## 1. Base paths and versions

| Path | Purpose |
|---|---|
| `/api/v1` | Native NEXORA REST contract |
| `/ws/v1/sessions/{id}` | Session event WebSocket (text JSON `EventEnvelope`) |
| `/ws/v1/sessions/{id}/audio` | Native audio delivery WebSocket (binary Opus packets) |
| `/nexora` | Lavalink-compatible adapter for client migration (`/nexora/info`, `/nexora/loadtracks`, `/nexora/sessions`, `/nexora/websocket`, plugin and streaming routes) |

All JSON request/response bodies use `application/json`. Error responses use a stable envelope:

```json
{ "error": { "code": "<code>", "message": "<message>" } }
```

## 2. Authentication

Routes require Bearer-token authorization when `NEXORA_SERVER_PASSWORD` (or `--password`) is set. When unset, authorization is disabled for local development only.

Routes that do **not** require authorization: `/health`, `/api/v1/info`, `/nexora/info`.

Authorized routes read the `Authorization: Bearer <token>` header and compare it to the configured password.

```
Authorization: Bearer <application-token>
```

## 3. Native REST API (`/api/v1`)

### 3.1 Health

**GET /health**

Health status. No auth required.

Response:

```json
{ "service": "nexora-gateway", "healthy": true, "message": "sessions=0" }
```

### 3.2 Info

**GET /api/v1/info**

Engine and API information. No auth required.

Response:

```json
{ "engine": "nexora", "api_version": "v1", "lavalink_compatible": true,
  "features": ["rest","websocket","sessions","source-resolution","opus-output"] }
```

### 3.3 Sources

**GET /api/v1/sources**

List available source adapter IDs.

Response:

```json
{ "sources": ["local", "radio", "http-stream"] }
```

### 3.4 Search

**POST /api/v1/search**

Search eligible sources and return ranked track references.

Request:

```json
{ "query": "example audio", "sources": ["local", "radio"], "limit": 10 }
```

Response:

```json
{ "results": [ { "track_id": "...", "source_id": "local", "title": "...", "duration_ms": 120000 } ] }
```

`limit` is capped at 50; default 10.

### 3.5 Session lifecycle

**POST /api/v1/sessions**

Create a session.

Request:

```json
{ "application": "my-bot", "version": "1.0.0",
  "output": { "sample_rate": 48000, "channels": 2, "codec": "opus" } }
```

Response (201 Created):

```json
{ "session_id": "nx_session_1", "status": "created", "engine": "nexora", "api_version": "v1" }
```

**GET /api/v1/sessions/{id}**

Read session state (status, position, volume).

Response:

```json
{ "session_id": "nx_session_1", "status": "playing", "position_ms": 900, "volume": 100 }
```

**POST /api/v1/sessions/{id}/play**

Resolve a track and start playback. Resolution happens **before** state mutation; if the track cannot be resolved the session is not marked playing and a `TrackFailed` event is emitted.

Request:

```json
{ "track_id": "track_1", "start_position_ms": 0 }
```

Response:

```json
{ "status": "playing" }
```

**POST /api/v1/sessions/{id}/pause**

Pause playback.

Response: `{ "status": "paused" }`

**POST /api/v1/sessions/{id}/resume**

Resume playback.

Response: `{ "status": "playing" }`

**POST /api/v1/sessions/{id}/stop**

Stop playback and clear the current track.

Response: `{ "status": "stopped" }`

**POST /api/v1/sessions/{id}/seek**

Seek to a position by packet index (`position_ms / 20`).

Request:

```json
{ "position_ms": 1000 }
```

Response: `{ "position_ms": 1000 }`

**POST /api/v1/sessions/{id}/volume**

Set output volume (0..150). Applied as PCM gain before encoding; values above 100 may clip.

Request:

```json
{ "volume": 100 }
```

Response: `{ "volume": 100 }`

### 3.6 Queue

**GET /api/v1/sessions/{id}/queue**

Read the current queue.

Response: `{ "queue": ["track_1", "track_2"] }`

**POST /api/v1/sessions/{id}/queue**

Enqueue a track (FIFO).

Request:

```json
{ "track_id": "track_2" }
```

Response: `{ "queue_length": 2 }`

### 3.7 Stream

**GET /api/v1/sessions/{id}/stream**

Batch-transcode the current local track into a temporary Ogg Opus file and return the bytes. The temp file is deleted after reading.

Response headers: `Content-Type: audio/ogg`. Body: raw Ogg Opus bytes.

### 3.8 Metrics

**GET /api/v1/metrics**

Runtime counters.

Response:

```json
{ "nexora_active_sessions": 1,
  "nexora_api_requests_total": 42,
  "nexora_search_requests_total": 5,
  "nexora_stream_bytes_total": 12345,
  "nexora_api_errors_total": 0 }
```

## 4. Native WebSocket contracts

### 4.1 Event WebSocket

**GET /ws/v1/sessions/{id}**

Text JSON event stream. Auth required. One `EventEnvelope` per message.

Event types: `session.created`, `session.ready`, `track.started`, `track.ended`, `track.failed`, `track.position`, `playback.paused`, `playback.resumed`, `playback.stopped`, `queue.updated`, `source.unavailable`, `engine.warning`.

`EventEnvelope` fields:

| Field | Type | Description |
|---|---|---|
| `event_id` | string | Unique event id |
| `version` | string | API version (`v1`) |
| `timestamp` | integer | Epoch milliseconds |
| `session_id` | string | Target session id |
| `event_type` | string | One of the event types above |
| `payload` | object | Event-specific payload |
| `correlation_id` | string | Correlation id (e.g. `api-v1`) |

Clients must reconnect and retrieve current session state; they should not depend exclusively on transient events.

### 4.2 Audio WebSocket

**GET /ws/v1/sessions/{id}/audio**

Binary Opus delivery. Auth required. One raw Opus packet per binary message; no JSON wrapper.

- Nominal rate: 50 packets/second (one per 20 ms).
- Order: WebSocket message order equals packet sequence.
- Gap handling: if the consumer lags and a `RecvError::Lagged(n)` is returned, the consumer skips `n` messages and continues; it never blocks the producer.
- Each message body is the raw Opus `payload` bytes.

The event WebSocket stays text-only; audio never shares the control channel.

## 5. Lavalink-v4-compatible adapter (`/nexora/*`)

The `/nexora/*` surface is a translation layer for Lavalink v4 clients. It is
served under the `/nexora/` prefix, not `/v4/`. `/api/v1` remains the durable
native contract, and [COMPATIBILITY.md](COMPATIBILITY.md) is the authoritative
per-route status.

| Endpoint | Method | Purpose |
|---|---|---|
| `/version` | GET | Plain-text server version |
| `/nexora/info` | GET | Server info (version, build time, source managers, filters, plugins) |
| `/nexora/stats` | GET | Runtime stats (`cpu.lavalinkLoad`, players, memory) |
| `/nexora/routeplanner/status` | GET | Route planner status (204 — no planner configured) |
| `/nexora/routeplanner/free/address` | POST | 204 placeholder |
| `/nexora/routeplanner/free/all` | POST | 204 placeholder |
| `/nexora/loadtracks` | GET | Load tracks by identifier; returns the v4 track shape |
| `/nexora/decodetrack` | GET | Decode one encoded track |
| `/nexora/decodetracks` | POST | Decode a batch of encoded tracks |
| `/nexora/sessions` | POST | Create a session (returns 201) |
| `/nexora/sessions/{id}` | PATCH | Patch session (`resuming`, `timeout`) |
| `/nexora/sessions/{id}/players` | GET | List players |
| `/nexora/sessions/{id}/players/{guildId}` | GET/PATCH/DELETE | Read, patch or destroy one player |
| `/nexora/websocket` | GET | Lavalink event socket (server push only) |
| `/nexora/sources/health` | GET | Source health snapshot |
| `/nexora/plugins` | GET | Loaded plugins and their manifests |
| `/nexora/plugins/{name}/call` | POST | Run one plugin function under the sandbox |
| `/nexora/loadstream` | GET | Raw PCM (WAV) stream, gated by `audio.raw_streaming` |
| `/nexora/trackstream/{encoded}` | GET | Opus transcode of one track |
| `/nexora/connection` | GET | Voice connection report (no voice gateway) |
| `/nexora/loadlyrics`, `/nexora/loadchapters` | GET | Empty payloads (no lyrics/chapters source) |

Key compatibility notes:

- `loadType` values are lowercase: `track`, `playlist`, `search`, `empty`, `error`.
- Tracks carry the v4 shape: `encoded`, `info`, `pluginInfo`, `userData`.
- `encoded` is an opaque NEXORA base64 JSON blob; it is **not** Lavalink's binary track encoding and must be round-tripped through NEXORA.
- Player responses report `connected: false` and `ping: -1` because NEXORA has no
  voice gateway; the voice transport itself (per-player UDP socket, RTP framing,
  AES-128-CTR payloads) is implemented and the client owns the gateway.
- The event socket is `/nexora/websocket`, not `/nexora/sessions/{id}`.

## 6. Versioned contract types

Versioned DTOs live in the `nexora-protocol` crate:

| Type | Purpose |
|---|---|
| `CreateSessionRequest` | `application`, `version`, `output: AudioFormat` |
| `CreateSessionResponse` | `session_id`, `status`, `engine`, `api_version` |
| `SessionState` | `session_id`, `status`, `position_ms`, `volume` |
| `PlayRequest` | `track_id`, `start_position_ms` |
| `HealthStatus` | `service`, `healthy`, `message` |
| `EventEnvelope` | `event_id`, `version`, `timestamp`, `session_id`, `event_type`, `payload`, `correlation_id` |
| `SessionEventType` | event type enum (snake_case serialized) |
| `Command` | internal command enum (`CreateSession`, `Play`, `Pause`, `Resume`, `Stop`, `Seek`, `SetVolume`) |

Internal gRPC contract (`nexora.v1.SessionService`) lives in `proto/nexora.proto` with RPCs `CreateSession`, `GetSession`, `Play`, `Pause`, `Stop`, backed by the same in-memory `SessionManager` as the HTTP surface.

## 7. Versioning policy

Public API contracts are versioned independently from implementation details. Breaking changes require a new version and migration notes. The base path `/api/v1` is the stable integration point; the `/nexora/*` Lavalink-compatible adapter may evolve independently.

## 8. Related documents

- `docs/ARCHITECTURE.md` — high-level system design and data flow.
- `docs/SYSTEM_DESIGN.md` — observed runtime behavior and invariants.
- `docs/API.md` — native `/api/v1`, audio WebSocket, and `/nexora/*` contracts (shorter reference).
- `docs/LAVALINK_ANALYSIS.md` — Lavalink protocol reference and deliberate non-goals.
- `crates/nexora-protocol` — Rust source of versioned DTOs.
- `proto/nexora.proto` — internal gRPC contract.
