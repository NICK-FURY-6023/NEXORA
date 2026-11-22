# NEXORA — Today's Execution Schedule

Manager: Hermes Agent. Goal: finish the remaining high-value increments today with parallel CLI workers and verified pushes after each completed wave.

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

## Wave 2 — manager completes or repairs worker output

- Review every worker diff; revert unapproved changes.
- Run each SDK's own test command.
- Integrate Rust core changes and re-run workspace checks.
- Push wave 2 with a conventional commit.

## Wave 3 — distributed + plugins

- Add `crates/nexora-cluster` registry/skeleton and tests.
- Add plugin manifest contract and sandboxed plugin boundary documentation; WASM runtime only if verifiable today.
- Push after green checks.

## Wave 4 — production hardening

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
