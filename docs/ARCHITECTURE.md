# NEXORA Architecture (implemented structure)

## Scope

This document tracks what exists after Phase 2. Product positioning: NEXORA is a from-scratch Rust alternative to hosted Lavalink, with a Lavalink-compatible server surface planned for migration and the universal `/api/v1` contract as the long-term integration point. The target product architecture remains in `README.md`; this file tracks the implemented structure.

## Workspace

```text
crates/nexora-error      -> structured errors, no secret leakage
crates/nexora-common     -> IDs, service names, API version, timestamps
crates/nexora-audio-types -> AudioFrame/AudioFormat/AudioCodec, FramePool contract
crates/nexora-protocol   -> versioned DTOs, event envelope, command contract
crates/nexora-config     -> Figment defaults + TOML + environment overrides
crates/nexora-telemetry  -> tracing init, metric names
crates/nexora-service    -> Service trait, health, shutdown
services/nexora-gateway  -> Phase 1 skeleton binary
```

### Audio engine (Phase 2)

```text
crates/nexora-audio-decode   -> Symphonia decoder to f32 PCM
crates/nexora-audio-codec    -> WAV writer, Opus packets, Ogg Opus writer
crates/nexora-audio-pipeline -> resampling, clipping, bounded queue, channel remap
services/nexora-audio-engine -> standalone CLI orchestrating decode -> resample -> encode
```

### Source framework (Phase 3)

```text
crates/nexora-source-api       -> AudioSource trait, auth/track/resolution contracts
crates/nexora-source-framework -> registry, ranking, timeout, cancellation, adapters
```

### Hosted server (Phase 4a)

```text
crates/nexora-session         -> session lifecycle, queue, event broadcast
services/nexora-server        -> Axum REST + WebSocket, /api/v1 and /v4 surfaces
```

## Dependency direction

```text
nexora-error -> nexora-common -> nexora-audio-types
                     |                 |
                     +-> nexora-protocol
                              |
nexora-config / nexora-telemetry
                     |
                nexora-service
                     |
             nexora-gateway
```

No cycles. Shared types do not depend on services. The audio-types crate does not depend on protocol or config.

## Control plane and data plane

Phase 1 provided the foundation. Phase 2 adds the standalone data plane: local file input -> decode -> resample -> channel remap -> encode -> output file. The approved split for later phases:

- Control plane: gateway, auth, registry, resolver, session, configuration, cluster.
- Data plane: source-worker, decoder, audio-engine, delivery, buffer manager.

Audio frames must never be routed through the gateway. Gateway owns commands and events; delivery workers own audio transport.

## Security posture

- No secrets in code, config defaults, logs, or errors.
- No network listeners in either binary.
- No provider integrations, external media access, or open-proxy behavior.
- No unsafe Rust.
- Dependency versions are pinned in the workspace manifest and locked in `Cargo.lock`.

## Deferred to later phases

Internal gRPC and full Lavalink client conformance (Phase 4 remainder), SDKs (Phase 5), clustering (Phase 6), WASM plugins (Phase 7), and production hardening (Phase 8).
