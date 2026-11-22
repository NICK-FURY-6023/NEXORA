# ADR-016: Lavalink-compatible surface over native API

## Context
Existing Discord bots already speak Lavalink's REST/WebSocket protocol. NEXORA also needs a universal `/api/v1` contract.

## Decision
Keep `/api/v1` as the native API and add a `/v4` compatibility layer for migration.

## Consequences
Bots can point at NEXORA with minimal client changes. Compatibility surface must be tested separately from the native API.

## Alternatives
Replacing all clients with NEXORA SDKs was rejected because it blocks migration.

## Status
Accepted; partial `/v4` surface implemented in Phase 4a. Full client conformance remains.
