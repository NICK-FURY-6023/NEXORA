# NEXORA API Contracts

## Phase 1 status

Phase 4a implements REST and WebSocket endpoints in `nexora-server`. gRPC remains planned.

NEXORA API: `/health`, `/api/v1/info`, `/api/v1/sources`, `/api/v1/search`, session create/read/play/pause/resume/stop/seek/volume/queue/stream, `/api/v1/metrics`, and `/ws/v1/sessions/{id}`.

Lavalink-compatible surface: `/v4/info`, `/v4/loadtracks`, `/v4/sessions`, `/v4/sessions/{id}/player`, and `/v4/sessions/{id}` WebSocket events.

Versioned contract types live in the `nexora-protocol` crate:

- `CreateSessionRequest` / `CreateSessionResponse`
- `SessionState`
- `PlayRequest`
- `HealthStatus`
- `EventEnvelope`
- `SessionEventType`
- `Command`

These contracts now back the implemented HTTP surface; contract tests cover the JSON shapes used by the server.

## Planned endpoint groups

See `README.md` section 7 for the complete planned list: health/info, sources, search, resolve, sessions, playback controls, filters, queue, stream, and metrics. Base path: `/api/v1/`.

## Planned event types

`session.created`, `session.ready`, `track.started`, `track.ended`, `track.failed`, `track.position`, `playback.paused`, `playback.resumed`, `playback.stopped`, `queue.updated`, `source.unavailable`, `engine.warning`.

Every event envelope carries an event ID, version, timestamp, session ID, event type, payload, and correlation ID.

## Versioning policy

Public API contracts are versioned independently from implementation details. Breaking changes require a new version and migration notes.
