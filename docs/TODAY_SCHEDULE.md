# NEXORA — Today's Execution Schedule

Manager: Hermes Agent. Goal: finish the remaining high-value increments today with parallel CLI workers and verified pushes after each completed wave.

Current state (2026-11-27): the waves below are historical. M0-M10 are
implemented and verified at commit `52f0b95` with 354 passing tests and CI run
`45974612022` green. The remaining work is tracked in
[PROJECT_STATUS.md](PROJECT_STATUS.md) and
[COMPATIBILITY.md](COMPATIBILITY.md).

## Wave 0 — baseline (done)

- Phase 1 foundation — `8313fa4`
- Phase 2 audio engine — `9872c76`
- Phase 3 source framework — `7ed6b6e`
- Phase 4a REST/WebSocket — `b8d8e84`
- Phase 4b internal gRPC — `7619e60`

## Wave 1 — parallel SDK + deployment artifacts

| Owner | Scope | Gate |
|---|---|---|
| `fcc-opencode` | `sdks/python/**` only | Python SDK + `unittest` pass |
| `fcc-pi` | `sdks/nodejs/**` only | Node SDK + `node --test` pass |
| `fcc-agy` | `deploy/**`, `docs/DEPLOYMENT.md` only | Docker/compose files parse by inspection; no root manifest edits |
| Manager | Rust core: runtime counters, Lavalink conformance endpoints | `cargo fmt/clippy/test` green |

No worker may edit root `Cargo.toml`, existing `crates/**`, `services/**`, `README.md`, or other workers' scopes.

## Wave 2 — completed

- OpenCode produced Java/Go/C#/PHP SDK sources; toolchains for those languages are not installed on this host, so they are source-only and unverified by compilers.
- Pi claimed ADR files but wrote none; manager wrote ADR-015..020 directly.
- Agy failed with a gateway error; manager wrote monitoring examples directly.
- Python 6 tests and Node 5 tests pass; Rust workspace 70 tests pass.

## Wave 3 — completed

- `crates/nexora-cluster`: worker registry, capability filtering, weighted scheduler, 2 tests.
- `crates/nexora-plugin-api`: manifest validation and restrictive sandbox policy, 3 tests.
- WASM runtime execution remains deferred; the contract and limits are explicit.

## Wave 4 — completed

- Security review of HTTP auth, SSRF validation, secrets, and dependency licenses.
- Update `docs/PROJECT_STATUS.md`, `docs/ARCHITECTURE.md`, `docs/API.md`, `CHANGELOG.md`, and README.
- Final full verification: fmt, clippy, tests, HTTP smoke, gRPC smoke, SDK tests.
- Final push and handoff.

## Rules

1. Every phase push only after real command output is green.
2. No secrets in code, docs, or commits.
3. No orphan CLI workers; stop and verify before staging.
4. Worker output is evidence, not truth — manager re-runs decisive checks.
5. If a worker stalls, kill it and take over its scope directly.

Final chain: `8313fa4` -> `9872c76` -> `7ed6b6e` -> `b8d8e84` -> `7619e60` -> `765e0e1` -> `d408d5f` -> `ca44509` -> final hardening push. Every phase was verified before the next started.
