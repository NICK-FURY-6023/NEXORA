# Architecture Decision Records

The following decisions were approved in `README.md` and are tracked here as the initial ADR index. New decisions must be added as separate records with context, decision, consequences, and alternatives.

| ADR | Decision | Status |
|---|---|---|
| ADR-001 | Rust as primary implementation language | Accepted |
| ADR-002 | Distributed microservices | Accepted |
| ADR-003 | Separate control plane and data plane | Accepted |
| ADR-004 | Tokio asynchronous runtime | Accepted |
| ADR-005 | gRPC for internal RPC | Accepted (Phase 4+) |
| ADR-006 | REST + WebSocket for public APIs | Accepted (Phase 4+) |
| ADR-007 | Native audio pipeline | Accepted (Phase 2+) |
| ADR-008 | Independent source adapter interfaces | Accepted (Phase 3+) |
| ADR-009 | WASM sandbox for third-party plugins | Accepted (Phase 7+) |
| ADR-010 | PostgreSQL for persistent metadata | Accepted (later phase) |
| ADR-011 | NATS for service events | Accepted (Phase 6+) |
| ADR-012 | Docker-first deployment | Accepted (later phase) |
| ADR-013 | OpenAPI and protobuf as API contracts | Accepted (Phase 4+) |
| ADR-014 | Benchmark-driven performance optimization | Accepted |
| ADR-015 | From-scratch Rust core, no JVM dependency | Accepted |
| ADR-016 | Native `/api/v1` plus `/v4` compatibility surface | Accepted |
| ADR-017 | In-process session state | Accepted |
| ADR-018 | gRPC internal session service | Accepted |
| ADR-019 | Pure-Rust Opus encoding | Accepted |
| ADR-020 | Bounded audio queues | Accepted |
| ADR-021 | Repository documentation layout | Accepted |
| ADR-022 | Playback pipeline architecture | Accepted |

Concrete ADR files exist for ADR-015..022; ADR-001..014 remain the accepted
architecture index from the original plan.

See [docs/README.md](../README.md) for the reader-facing navigation map.
