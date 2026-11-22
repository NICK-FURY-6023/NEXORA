# ADR-019: Pure Rust Opus

## Context
Opus encoding must work without a system `libopus` dependency.

## Decision
Use `opus-rs` for encoding and the `ogg` crate for the container.

## Consequences
Simpler deployment and no native library matrix. Codec fidelity must be validated against reference Opus where available.

## Alternatives
FFI to `libopus` was rejected because of deployment complexity.

## Status
Accepted and implemented in Phase 2.
