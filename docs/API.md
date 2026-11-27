# NEXORA API Contracts

## Implemented status

The REST and WebSocket endpoints are implemented in `nexora-server`; internal
gRPC is `nexora.v1.SessionService`.

NEXORA API: `/health`, `/api/v1/info`, `/api/v1/sources`, `/api/v1/search`, session create/read/play/pause/resume/stop/seek/volume/queue/stream, `/api/v1/metrics`, and `/ws/v1/sessions/{id}`.

Lavalink-compatible surface (served under the `/nexora/*` prefix, not `/v4/*`): `/version`, `/nexora/info`, `/nexora/stats`, `/nexora/loadtracks`, `/nexora/decodetrack`, `/nexora/decodetracks`, `/nexora/sessions`, `/nexora/sessions/{id}/players`, `/nexora/sessions/{id}/players/{guildId}`, the `/nexora/routeplanner/*` routes, and the `/nexora/websocket` event socket. The exact per-route state is in [COMPATIBILITY.md](COMPATIBILITY.md).

The `/nexora/loadtracks` response uses the v4 track shape (`encoded`, `info`, `pluginInfo`, `userData`). `encoded` is an opaque NEXORA base64 JSON blob; it is not Lavalink's binary track encoding and must be round-tripped through NEXORA.

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

## Implemented endpoint groups

The native contract is live under `/api/v1/`: health/info, sources, search,
sessions, playback controls (play/pause/resume/stop/seek/volume), queue, raw
PCM stream and metrics. Filters are configured on the Lavalink-compatible
player route (`PATCH /nexora/sessions/{id}/players/{guildId}`); there is no
native `/api/v1/.../filters` or `/api/v1/resolve` route. The compatibility
surface adds the `/nexora/*` v4 routes, session resume,
the plugin endpoints, and the voice RTP transport. `GET /metrics` serves the
same real counters as `/api/v1/metrics` in Prometheus text format
(`text/plain; version=0.0.4`) under the same authorization. See
[COMPATIBILITY.md](COMPATIBILITY.md) for the exact per-route status.

## Implemented event types

The event WebSocket emits NEXORA's typed envelope (`event`, `sessionId`,
`type`, `payload`, `timestamp`, `eventId`, `version`, `correlationId`). The
Lavalink-compatible `playerUpdate` frames carry position, time, connected and
ping. Source-health changes use the `SourceHealth` DTO (`sourceId`, `healthy`,
`detail`, `checkedAt`). The full event list and the deliberate omissions are
in [COMPATIBILITY.md](COMPATIBILITY.md).

## Versioning policy

Public API contracts are versioned independently from implementation details. Breaking changes require a new version and migration notes.
