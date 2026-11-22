# ADR-017: In-process session state

## Context
Phase 4 needs working sessions before distributed persistence exists.

## Decision
Store sessions, queues, and playback state in an in-memory `SessionManager` behind a stable API.

## Consequences
Fast local operation and simple tests. State is lost on restart and horizontal scaling needs a later shared store.

## Alternatives
Introducing a database in Phase 4 was rejected as premature.

## Status
Accepted and implemented in Phase 4a. Durable state is a later phase.
