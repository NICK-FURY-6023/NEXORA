# NEXORA API Contracts

## Phase 1 status

Phase 4a implements REST and WebSocket endpoints in `nexora-server`. Phase 4b adds internal gRPC via `nexora.v1.SessionService`.

NEXORA API: `/health`, `/api/v1/info`, `/api/v1/sources`, `/api/v1/search`, session create/read/play/pause/resume/stop/seek/volume/queue/stream, `/api/v1/metrics`, and `/ws/v1/sessions/{id}`.

Lavalink-compatible surface: `/v4/info`, `/v4/loadtracks`, `/v4/sessions`, `/v4/sessions/{id}/player`, and `/v4/sessions/{id}` WebSocket events.

The `/v4/loadtracks` response uses the v4 track shape (`encoded`, `info`, `pluginInfo`, `userData`). `encoded` is an opaque NEXORA base64 JSON blob; it is not Lavalink's binary track encoding and must be round-tripped through NEXORA.

Versioned contract types live in the `nexora-protocol` crate:

- `CreateSessionRequest` / `CreateSessionResponse`
- `SessionState`
- `PlayRequest`
- `HealthStatus`
- `EventEnvelope`
- `SessionEventType`
- `Command`

These contracts now back the implemented HTTP surface; contract tests cover the JSON shapes used by the server.

## Native audio delivery

`GET /ws/v1/sessions/{id}/audio` is the data-plane WebSocket. It uses the same
Bearer authorization as the REST API. Each binary message is one raw 20 ms Opus
packet; message order is packet order. The nominal rate is 50 packets/second.
Consumers that lag skip missed broadcasts rather than blocking the producer.
The event WebSocket at `/ws/v1/sessions/{id}` stays text-only.

## Planned endpoint groups

See `README.md` section 7 for the complete planned list: health/info, sources, search, resolve, sessions, playback controls, filters, queue, stream, and metrics. Base path: `/api/v1/`.

## Planned event types

`session.created`, `session.ready`, `track.started`, `track.ended`, `track.failed`, `track.position`, `playback.paused`, `playback.resumed`, `playback.stopped`, `queue.updated`, `source.unavailable`, `engine.warning`.

Every event envelope carries an event ID, version, timestamp, session ID, event type, payload, and correlation ID.

## Versioning policy

Public API contracts are versioned independently from implementation details. Breaking changes require a new version and migration notes.
