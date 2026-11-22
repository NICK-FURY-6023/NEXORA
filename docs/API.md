# NEXORA API Contracts

## Phase 1 status

No public HTTP, WebSocket, or gRPC endpoint is implemented in Phase 1. The gateway binary is a skeleton.

Versioned contract types live in the `nexora-protocol` crate:

- `CreateSessionRequest` / `CreateSessionResponse`
- `SessionState`
- `PlayRequest`
- `HealthStatus`
- `EventEnvelope`
- `SessionEventType`
- `Command`

These are Serde-serializable Rust contracts and have round-trip tests. They are the basis for the Phase 4 API, not a published wire API yet.

## Planned endpoint groups

See `README.md` section 7 for the complete planned list: health/info, sources, search, resolve, sessions, playback controls, filters, queue, stream, and metrics. Base path: `/api/v1/`.

## Planned event types

`session.created`, `session.ready`, `track.started`, `track.ended`, `track.failed`, `track.position`, `playback.paused`, `playback.resumed`, `playback.stopped`, `queue.updated`, `source.unavailable`, `engine.warning`.

Every event envelope carries an event ID, version, timestamp, session ID, event type, payload, and correlation ID.

## Versioning policy

Public API contracts are versioned independently from implementation details. Breaking changes require a new version and migration notes.
