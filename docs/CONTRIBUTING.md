# Contributing to NEXORA

## Before opening a pull request

1. Read [the documentation map](README.md), the root `README.md`, `docs/ARCHITECTURE.md`, `docs/PROTOCOL.md`, and `docs/MASTER_AI_INSTRUCTIONS.md`.
2. Keep changes scoped to the current milestone.
3. Use safe Rust; document and test any `unsafe` with a safety argument.
4. Add tests for every implemented component.
5. Run the baseline checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

6. Update `docs/PROJECT_STATUS.md`, `CHANGELOG.md`, relevant docs, and the [documentation map](README.md) when adding a new doc.
7. Never commit `.env`, credentials, tokens, or local databases.

## Where does a change belong?

| Change | Responsible place | Proof |
|---|---|---|
| New REST or WS route | `services/nexora-server/src/lib.rs` | `services/nexora-server/tests/http.rs` |
| Session/queue/playback invariant | `crates/nexora-session` | `crates/nexora-session/tests/session.rs` |
| Source adapter or SSRF rule | `crates/nexora-source-framework` | `crates/nexora-source-framework/tests/framework.rs` |
| Decode/resample/encode behavior | `crates/nexora-audio-*` | crate tests + `cargo bench` where relevant |
| Public DTO shape | `crates/nexora-protocol` | round-trip tests + `docs/PROTOCOL.md` |
| Lavalink wire compatibility | `/v4` handler in `nexora-server` | contract test + `docs/LAVALINK_ANALYSIS.md` |
| Deployment/monitoring | `deploy/` | `docs/DEPLOYMENT.md` |
| Architecture decision | `docs/adr/` | new ADR, never rewrite old ADRs |

## Architecture decisions

Important architecture changes must be recorded in `docs/adr/`. The initial decisions are summarized in `docs/adr/README.md`.

## Commit style

Use Conventional Commits: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, `perf:`, `ci:`.

## AI agent engineering guide

AI coding agents must treat the repository as an engineering project, not as permission to rewrite the architecture without review.

Before changing code, an agent should:

1. Read the repository instructions and relevant documentation.
2. Inspect the actual directory structure, Cargo workspace, and existing implementation.
3. Check Git status and avoid overwriting unrelated user changes.
4. Identify what is implemented, partially implemented, and only planned.
5. Report architectural conflicts and missing prerequisites.
6. Propose a small, testable implementation plan.
7. Wait for approval before major architectural changes or destructive operations.
8. Implement incrementally and run the relevant tests.
9. Update documentation to match actual behavior.
10. Never invent successful test results, benchmarks, integrations, or completed features.

Master instructions: `docs/MASTER_AI_INSTRUCTIONS.md`.

## Security

- No secrets in code, config defaults, logs, or errors.
- No provider integrations, external media access, or open-proxy behavior.
- No unsafe Rust unless a benchmark and safety review justify it.
- Dependency versions are pinned in the workspace manifest and locked in `Cargo.lock`.

## Verification

For each change:

1. RED contract test for the new wire/behavior invariant.
2. Implement in the owning crate only.
3. `cargo fmt --all -- --check`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `cargo test --workspace`
6. Live server smoke (when touching the server or audio path): start `nexora-server` with a temporary media root, create a session, play a generated WAV, connect the audio WebSocket, and record real packet count/timing; stop the process afterwards.
7. Update docs with exact evidence and commit the change.
