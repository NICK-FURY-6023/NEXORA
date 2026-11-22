# NEXORA Architecture (implemented structure)

## Scope

This document describes what exists after Phase 1. The target product architecture remains in `README.md`; this file tracks the implemented structure.

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

Phase 1 implements only foundation types and a gateway skeleton. The approved split for later phases:

- Control plane: gateway, auth, registry, resolver, session, configuration, cluster.
- Data plane: source-worker, decoder, audio-engine, delivery, buffer manager.

Audio frames must never be routed through the gateway. Gateway owns commands and events; delivery workers own audio transport.

## Security posture in Phase 1

- No secrets in code, config defaults, logs, or errors.
- No network listeners in the gateway skeleton.
- No provider integrations and no external media access.
- No unsafe Rust.
- Dependency versions are pinned in the workspace manifest and locked in `Cargo.lock`.

## Deferred to later phases

Audio decoding (Phase 2), source adapters and SSRF validation (Phase 3), HTTP/WebSocket/gRPC APIs (Phase 4), SDKs (Phase 5), clustering (Phase 6), WASM plugins (Phase 7), and production hardening (Phase 8).
