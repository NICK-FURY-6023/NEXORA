# NEXORA Remaining Work Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the verified remaining backlog from `docs/PROJECT_STATUS.md` without changing deliberate non-goals from `docs/COMPATIBILITY.md`.

**Architecture:** Each task is an independently testable slice. A task may only touch the files named in its own task block plus the current-facing docs named in Task 8. Root manifests and CI are off-limits unless the task says otherwise. Every task ends with a commit and a fresh full-gate result.

**Tech Stack:** Rust 1.100.0, Axum, Tokio, wasmi, Criterion, mdBook-compatible Markdown, GitHub Actions.

**Spec:** `docs/PROJECT_STATUS.md`, `docs/COMPATIBILITY.md`, `docs/BENCHMARKS.md`, `docs/SECURITY_REVIEW.md`.

## Global Constraints

- Toolchain is `rustc 1.100.0 (0ef1bc386 2026-11-09)`; keep `rust-toolchain.toml` unchanged.
- Direct dependencies stay exact-pinned in `Cargo.toml`; commit any `Cargo.lock` diff with the crate that caused it.
- Never add a third-party source library, Java plugin runtime, hosted resolver, or credential to the repository.
- Never log, embed, or commit real tokens, passwords, or provider credentials.
- TDD is mandatory: write the failing test, watch it fail, then write minimal code.
- Full gate for every commit: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Verify the pushed commit's CI run before claiming the task complete.
- Deliberate `not-targeted` rows in `docs/COMPATIBILITY.md` are out of scope, not backlog.
- The human partner has repeatedly authorized direct work and pushes on `main` in this repository.

## Review Focus

- Auth must apply to every new HTTP endpoint, including `/metrics`.
- Empty counters must still render valid output before any traffic arrives.
- A plugin manifest must never escalate a host-granted capability.
- Durable sessions must not resurrect a session after an explicit stop.
- Cluster failover must not route to a worker that has been marked down.
- Credentialed provider tests must stay opt-in and must never store real credentials.
- Docs must not advertise a capability whose test suite was not run.

---

### Task 1: Verify and commit the plugin capability WIP

**Files:**
- Modify: `crates/nexora-plugin-api/src/runtime.rs`
- Modify: `crates/nexora-plugin-api/src/sandbox.rs`
- Test: `crates/nexora-plugin-api/tests/capabilities.rs`

**Interfaces:**
- Consumes: `effective_policy(manifest, approved) -> NexoraResult<SandboxPolicy>`
- Produces: no new public API; only the corrected enforcement of manifest capabilities.

- [ ] **Step 1: Run the package tests and read the output**

Run: `cargo test -p nexora-plugin-api --test capabilities`
Expected: PASS; if it fails, use systematic debugging before changing code.

- [ ] **Step 2: Run the package clippy gate**

Run: `cargo clippy -p nexora-plugin-api --all-targets -- -D warnings`
Expected: currently FAIL with an unused import (`PluginSandbox`); remove the unused import in `runtime.rs`, not the capability tests.

- [ ] **Step 3: Re-run the package gate**

Run: `cargo test -p nexora-plugin-api && cargo clippy -p nexora-plugin-api --all-targets -- -D warnings`
Expected: PASS, including `a_manifest_that_does_not_ask_for_the_network_may_not_reach_it`.

- [ ] **Step 4: Run the full gate and commit**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: PASS. Commit only the three plugin-api paths; do not touch other WIP.

- [ ] **Step 5: Verify pushed CI**

Run: `git push origin main`, then read the Actions run status via the API.
Expected: SUCCESS on the pushed SHA.

### Task 2: Prometheus exposition with real counters

**Files:**
- Modify: `services/nexora-server/src/lib.rs:40-75` (if accessors are needed)
- Modify: `services/nexora-server/src/lib.rs:415-435` (router)
- Modify: `services/nexora-server/src/lib.rs:1193-1207` (`metrics` handler)
- Test: `services/nexora-server/tests/compat/main.rs`

**Interfaces:**
- Consumes: `RuntimeMetrics::active_sessions()`, `request()`, `search()`, `streamed(bytes)`, and the existing `errors` counter.
- Produces: `GET /metrics`, same Bearer auth as `/api/v1/metrics`, `Content-Type: text/plain; version=0.0.4`, Prometheus text with `# TYPE` lines.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn compat_metrics_exposes_real_counters_in_prometheus_text() {
    let server = spawn_server().await;
    let response = get_with_auth(&server, "/metrics").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("# TYPE nexora_api_requests_total counter"));
    assert!(body.contains("nexora_active_sessions 0"));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p nexora-server --test compat compat_metrics_exposes_real_counters_in_prometheus_text`
Expected: FAIL with 404 or "assertion failed".

- [ ] **Step 3: Implement the minimal handler and route**

Add a `prometheus_metrics` handler that authorizes exactly like `metrics`, reads the existing atomics, and formats the five metric names already used in JSON: `nexora_active_sessions` (gauge), `nexora_api_requests_total`, `nexora_search_requests_total`, `nexora_stream_bytes_total`, `nexora_api_errors_total` (counters). Register `/metrics` next to `/api/v1/metrics`. Keep the JSON endpoint unchanged.

- [ ] **Step 4: Run test and full gate**

Run: the named test, then `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
Expected: PASS; the named test must pass after having failed in Step 2.

- [ ] **Step 5: Commit, push, verify CI**

Expected: SUCCESS on the pushed SHA.

### Task 3: Real-client conformance smoke suite

**Files:**
- Create: `services/nexora-server/tests/compat/clients.rs`
- Modify: `services/nexora-server/tests/compat/main.rs` (module registration only)

**Interfaces:**
- Consumes: the `/nexora/*` routes listed in `docs/COMPATIBILITY.md`.
- Produces: a test suite that drives one session through v4 info, loadtracks, player PATCH, WebSocket `playerUpdate`, and resume.

- [ ] **Step 1: Write a failing test that asserts a `playerUpdate` frame follows a seek**

Run: the new test file; expected FAIL until the harness is registered and the frame is asserted.

- [ ] **Step 2: Implement the harness with a real WebSocket client**

Use the existing compat server helper and a real client upgrade; no mocks. Assert the event envelope fields from `docs/PROTOCOL.md`.

- [ ] **Step 3: Add one test per partial surface**

`loadlyrics` empty payload, `loadchapters` empty array, `/connection` disabled shape. Each must document the current boundary, not pretend a source exists.

- [ ] **Step 4: Full gate, commit, push, verify CI**

Expected: PASS and CI SUCCESS.

### Task 4: Durable session state

**Files:**
- Create: `crates/nexora-session/src/durable.rs`
- Modify: `crates/nexora-session/src/lib.rs`
- Test: `crates/nexora-session/tests/durable.rs`

**Interfaces:**
- Consumes: `SessionManager` snapshots.
- Produces: `restore_snapshot(snapshot) -> SessionManager` and `snapshot() -> SessionSnapshot`.

- [ ] **Step 1: Write a failing test: a stopped session is not restored**

Run: `cargo test -p nexora-session --test durable`; expected FAIL.

- [ ] **Step 2: Implement JSON snapshot save/load under a configured path**

Use the project's existing error type; never serialize credentials or secrets.

- [ ] **Step 3: Add restart recovery test in the server compat suite**

Expected: a session restored from disk keeps queue and volume but does not resume playback automatically.

- [ ] **Step 4: Full gate, commit, push, verify CI**

### Task 5: Cluster failover

**Files:**
- Modify: `crates/nexora-cluster/src/lib.rs`
- Test: `crates/nexora-cluster/tests/failover.rs`

**Interfaces:**
- Consumes: worker registry and weighted scheduler from Task `ed22756`.
- Produces: `mark_down(worker_id)` and routing that excludes down workers.

- [ ] **Step 1: Write the failing test: routing avoids a down worker**

Expected FAIL before the implementation.

- [ ] **Step 2: Implement down-state with no new dependency**

Expected PASS; a cluster with no healthy workers returns a typed error.

- [ ] **Step 3: Full gate, commit, push, verify CI**

### Task 6: Opt-in credentialed provider harness

**Files:**
- Create: `crates/nexora-source-universal/tests/live_providers.rs`
- Modify: `docs/UNIVERSAL_SOURCES.md` (status note only)

**Interfaces:**
- Consumes: existing `TokenStore` and adapter parsers.
- Produces: ignored tests gated by environment variable; no credential values in code or logs.

- [ ] **Step 1: Write one ignored test per credentialed provider**

Each test reads its token from the environment, skips when absent, and asserts only that a real search returns a parseable result.

- [ ] **Step 2: Run with no credentials and confirm all tests skip**

Run: `cargo test -p nexora-source-universal --test live_providers`
Expected: 0 failures, all skipped.

- [ ] **Step 3: Full gate, commit, push, verify CI**

### Task 7: SDK toolchain verification

**Files:**
- Modify: `docs/PROJECT_STATUS.md` (status only)

**Interfaces:**
- Consumes: SDK sources in `sdks/`.
- Produces: a toolchain availability matrix with exact compiler/runtime versions where available.

- [ ] **Step 1: Probe each toolchain without installing anything**

Run `java -version`, `go version`, `dotnet --version`, `php -v` and record which exist.

- [ ] **Step 2: Compile only the SDKs whose toolchain exists**

Expected: PASS or an honest failure record; do not claim unverified SDKs as verified.

- [ ] **Step 3: Full gate, commit, push, verify CI**

### Task 8: Server-level capacity benchmark

**Files:**
- Create: `benches/server_load.rs` in the server crate or a dedicated bench crate
- Modify: `docs/BENCHMARKS.md`

**Interfaces:**
- Consumes: the running server's `/api/v1` and audio WebSocket.
- Produces: a reproducible workload with sessions, packets/s, and error count.

- [ ] **Step 1: Write a harness that starts the server and measures 100 sessions**

Expected: runs and prints real counts, not estimates.

- [ ] **Step 2: Publish methodology and results in `docs/BENCHMARKS.md`**

Expected: host, toolchain, command, sample count, and explicit non-production caveat.

- [ ] **Step 3: Full gate, commit, push, verify CI**

### Task 9: Documentation sync

**Files:**
- Modify: `README.md`, `docs/README.md`, `docs/PROJECT_STATUS.md`, `docs/SYSTEM_DESIGN.md`, `docs/DEVELOPMENT.md`, `docs/TODAY_SCHEDULE.md`, `docs/SUMMARY.md`

**Interfaces:**
- Consumes: each completed task's verified commit SHA, test count, and CI run ID.
- Produces: one consistent current baseline across all current-facing docs; historical slices keep their original numbers.

- [ ] **Step 1: Update the verified baseline after every task**

Expected: exact commit, test count, CI run ID, and newly closed backlog item.

- [ ] **Step 2: Run the link checker**

Run: `python scripts/check_links.py`
Expected: "All relative markdown links resolve."

- [ ] **Step 3: Commit docs only when co-agent source WIP is uncommitted**

Expected: docs commit contains no unrelated source files.

### Task 10: Final whole-branch verification

**Files:** none.

- [ ] **Step 1: Run the full gate on the exact staged tree**
- [ ] **Step 2: Verify the pushed commit's CI run is SUCCESS**
- [ ] **Step 3: Report closed items, deferred minors, and any blockers verbatim**
