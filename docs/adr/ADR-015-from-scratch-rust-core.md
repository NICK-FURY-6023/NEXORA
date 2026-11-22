# ADR-015: From-scratch Rust core

## Context
Lavalink is JVM-based and depends on external source libraries. NEXORA needs an independent audio foundation that does not inherit those constraints.

## Decision
Implement the audio core in Rust with native decoding, resampling, encoding, and bounded buffers.

## Consequences
No JVM or Lavalink server is required. More implementation work is owned by NEXORA.

## Alternatives
Wrapping Lavalink was rejected because it would keep the third-party dependency.

## Status
Accepted and implemented in Phase 2.
