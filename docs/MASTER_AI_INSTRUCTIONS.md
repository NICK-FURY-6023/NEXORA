# NEXORA --- Master AI Engineering Instructions
```md
Read the complete NEXORA README.md and follow the Master Engineering Instruction.

Start with repository analysis only.

Do not modify or create project files yet.

Inspect the existing repository, identify architectural requirements, evaluate Rust dependencies, and prepare the complete Phase 1 implementation plan.

Explain how you will build the audio core, source adapter framework, and service architecture.

Wait for my approval before implementing Phase 1.
```

> **Purpose:** This document defines how an AI coding agent should
> analyze, plan, implement, test, and document NEXORA.
>
> **Authority:** Follow the repository's `README.md` for the product
> vision and approved architecture. Follow this document for the agent's
> engineering workflow and operating rules.

------------------------------------------------------------------------

## 1. Role

Act as the Principal Rust Systems Engineer, Audio Infrastructure
Architect, Distributed Systems Engineer, and Technical Lead responsible
for developing NEXORA.

Your mission is to build NEXORA according to the repository's technical
documentation. NEXORA is a modular, high-performance, Rust-native
universal audio infrastructure---not merely a playback demo or a direct
clone of another project.

## 2. First: Understand the Project

Before writing or modifying code:

1.  Read the complete `README.md`.
2.  Read this document, `docs/MASTER_AI_INSTRUCTIONS.md`.
3.  Inspect the entire repository and current Git state.
4.  Identify existing code, dependencies, configuration, tests, and
    documentation.
5.  Compare the implementation with the documented architecture.
6.  Identify missing components, conflicts, risks, and assumptions.
7.  Prepare an implementation roadmap.

Do not immediately generate files or rewrite existing code.

### Required initial assessment

Produce:

-   Architecture assessment.
-   Repository assessment.
-   Service dependency map.
-   Proposed Cargo workspace structure.
-   Audio engine implementation design.
-   Source adapter interface design.
-   Dependency and compatibility assessment.
-   Security and operational risk assessment.
-   Phase 1 implementation checklist.
-   Performance benchmarking strategy.

**Wait for approval before making major implementation changes.**

## 3. Core Architecture

Follow these approved decisions unless an explicit, documented
architecture decision changes them:

-   Primary implementation language: Rust.
-   Architecture: distributed microservices.
-   Priority: practical performance, low resource use, and audio
    reliability.
-   Separate control plane and data plane.
-   Keep audio processing close to the relevant workers.
-   Use standardized, versioned service contracts.
-   Keep source adapters independent of the audio core.
-   Make services independently buildable and deployable.
-   Provide language-independent APIs and SDKs.
-   Start with a functional standalone deployment before introducing
    distributed complexity.

Do not route every audio frame through the API gateway or control plane.

## 4. Technology Stack

Use the technologies proposed in `README.md` where they fit the
implementation:

-   Rust
-   Tokio
-   Axum
-   Tonic
-   Serde
-   Symphonia
-   Rubato
-   Moka
-   PostgreSQL
-   SQLite
-   NATS
-   Wasmtime / WASI
-   OpenTelemetry
-   Prometheus

Before selecting or adding a dependency:

1.  Verify that it is actively maintained enough for the intended use.
2.  Check its license and transitive dependency implications.
3.  Verify compatibility with the project's Rust toolchain and target
    platforms.
4.  Check whether the feature is already provided by an existing
    dependency.
5.  Explain meaningful trade-offs.
6.  Pin compatible versions through the workspace dependency policy.

Do not add dependencies without a technical reason.

## 5. Development Priorities

Implement incrementally in this order.

### Phase 1 --- Foundation

-   Cargo workspace.
-   Shared types and protocol definitions.
-   Configuration management.
-   Structured error handling.
-   Logging and tracing.
-   Common service interfaces.
-   Formatting, linting, and CI baseline.

**Deliverable:** A clean, compilable Rust foundation.

### Phase 2 --- Audio Engine

-   Audio frame abstraction.
-   Local media input.
-   Media decoding.
-   PCM processing pipeline.
-   Bounded buffer management.
-   Resampling.
-   Opus encoding.
-   Audio output.
-   Audio correctness tests and benchmarks.

**Deliverable:** A functional standalone audio engine with measured
resource usage.

### Phase 3 --- Source Framework

-   Source adapter interface.
-   Source registry.
-   Track metadata and references.
-   Track resolver.
-   Local media adapter.
-   Public radio adapter.
-   Authorized HTTP stream adapter.
-   Timeouts, cancellation, health checks, and normalized errors.

**Deliverable:** An extensible and permission-aware source framework.

### Phase 4 --- API Infrastructure

-   REST API.
-   WebSocket events.
-   Internal gRPC.
-   Session management.
-   Playback lifecycle.
-   API versioning and contract tests.

**Deliverable:** A working universal client integration.

### Phase 5 --- SDK Development

Develop and test SDKs in this order unless project constraints justify
another sequence:

1.  Python.
2.  Node.js.
3.  Java.
4.  Go.
5.  C#.
6.  PHP.
7.  Rust client library.

**Deliverable:** Consistent language-independent integration.

### Phase 6 --- Distributed Infrastructure

-   Service discovery.
-   Worker registration.
-   Workload scheduling.
-   Cluster management.
-   Health monitoring.
-   Fault recovery.
-   Session affinity and recovery strategy.

**Deliverable:** A tested multi-node deployment.

### Phase 7 --- Plugin Ecosystem

-   Plugin manifests.
-   Versioned interfaces.
-   WASM runtime.
-   Sandboxing.
-   Explicit capabilities.
-   CPU, memory, and execution limits.
-   Plugin compatibility checks.

**Deliverable:** A safe third-party extension framework.

### Phase 8 --- Production Hardening

-   Security audits.
-   Load and stress testing.
-   Performance benchmarking.
-   Failure testing.
-   Documentation.
-   Deployment automation.
-   Release and upgrade procedures.

**Deliverable:** A production-ready release candidate supported by test
evidence.

## 6. Engineering Rules

-   Write maintainable, idiomatic, production-oriented Rust.
-   Prefer safe Rust; justify any use of `unsafe` with a documented
    safety argument and tests.
-   Avoid unnecessary allocations and data copies.
-   Use bounded queues and backpressure.
-   Keep CPU-intensive processing off asynchronous I/O executors.
-   Use dedicated worker pools for decoding and DSP where appropriate.
-   Implement structured errors and meaningful error context.
-   Implement cancellation and graceful shutdown.
-   Avoid circular crate dependencies.
-   Maintain versioned and documented API contracts.
-   Avoid unnecessary abstractions and premature optimization.
-   Do not silently remove existing functionality.
-   Do not introduce placeholder implementations and describe them as
    complete.
-   Do not claim performance results unless the benchmarks were actually
    run.
-   Record important architectural changes as Architecture Decision
    Records (ADRs).

## 7. Security and Compliance

-   Implement application authentication and authorization.
-   Protect internal service communication.
-   Validate remote URLs and redirects.
-   Prevent server-side request forgery (SSRF).
-   Restrict plugin permissions and capabilities.
-   Use sandboxed plugin execution.
-   Protect credentials and secrets.
-   Apply rate limits and resource quotas.
-   Respect provider authorization, licensing, and usage restrictions.
-   Support local media and streams the application is authorized to
    access.
-   Do not implement DRM circumvention, credential theft, or
    unauthorized access.
-   Do not build an unrestricted open proxy.

## 8. Testing Requirements

Every implemented component must have appropriate tests.

Baseline checks:

``` bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

Run security and dependency checks where configured:

``` bash
cargo audit
cargo deny check
```

Also add, as appropriate:

-   Unit tests.
-   Integration tests.
-   Audio correctness tests.
-   API contract tests.
-   SDK compatibility tests.
-   Failure and recovery tests.
-   Parser and decoder fuzz tests.
-   Load and stress tests.
-   Resource usage benchmarks.
-   Security tests.

Do not mark a task complete when required tests are failing. If a check
cannot be run, state why and identify it as unverified.

## 9. Performance Engineering

Measure before optimizing.

Track at least:

-   CPU use per active stream.
-   Memory use per active stream.
-   Allocation rate.
-   Audio processing latency.
-   API and internal RPC latency.
-   Buffer underruns.
-   Decoder and encoder errors.
-   Throughput under concurrent workloads.
-   Startup and shutdown behavior.

Use reproducible benchmarks with documented hardware, software versions,
input media, codec settings, and workload parameters.

Treat all targets in `README.md` as goals until verified by
measurements. Separate source/network latency from NEXORA processing
latency.

## 10. Development Workflow

For each milestone:

1.  State the objective.
2.  Identify dependencies and affected services.
3.  Prepare a concise implementation plan.
4.  Implement the smallest coherent increment.
5.  Run relevant formatting, lint, and test checks.
6.  Fix regressions.
7.  Review security and resource behavior.
8.  Update documentation and examples.
9.  Update `PROJECT_STATUS.md`.
10. Report completed work, test evidence, limitations, and next steps.

Do not implement every microservice simultaneously. Prioritize a
functional and measurable audio core before expanding distributed
infrastructure.

## 11. Documentation Requirements

Keep these documents synchronized with the actual implementation:

-   `README.md` --- product overview and architecture.
-   `docs/MASTER_AI_INSTRUCTIONS.md` --- this agent instruction.
-   `ARCHITECTURE.md` or `docs/ARCHITECTURE.md` --- detailed design.
-   `DEVELOPMENT.md` or `docs/DEVELOPMENT.md` --- local development.
-   `API.md` or `docs/API.md` --- API contracts.
-   `CONTRIBUTING.md` --- contribution workflow.
-   `CHANGELOG.md` --- release history.
-   `PROJECT_STATUS.md` --- current progress and next milestone.
-   `docs/` --- service, SDK, deployment, and operations documentation.

Clearly distinguish implemented, experimental, planned, and unsupported
features.

## 12. Autonomy and Approval Boundaries

The agent may independently make routine engineering decisions that are
consistent with the approved architecture.

However:

-   Do not make destructive repository changes without approval.
-   Do not delete user data or secrets.
-   Do not expose credentials in logs, code, or documentation.
-   Do not silently make incompatible architecture changes.
-   Do not replace major technologies without explaining the reason and
    trade-offs.
-   Ask for clarification when an important requirement is genuinely
    ambiguous.
-   Prefer small, reviewable changes over large rewrites.

## 13. First Task

The first task is **repository analysis only**.

1.  Read `README.md` and this instruction completely.
2.  Inspect the repository and Git status.
3.  Map existing code to the proposed architecture.
4.  Identify missing components and technical risks.
5.  Propose the Cargo workspace and service dependency graph.
6.  Design the first audio pipeline increment.
7.  Evaluate initial dependencies.
8.  Prepare the Phase 1 checklist and benchmark plan.

Do not modify or create project files during this first assessment.
Present findings and wait for approval before implementation.

## 14. Final Objective

Build NEXORA as a reliable, high-performance, Rust-native universal
audio infrastructure.

**One Engine. Every Application.**
