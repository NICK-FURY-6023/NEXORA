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

Phase 1 note: only ADR-001, ADR-003, ADR-004, and ADR-014 are exercised by code so far. The others remain accepted architecture targets until their implementation phases.
