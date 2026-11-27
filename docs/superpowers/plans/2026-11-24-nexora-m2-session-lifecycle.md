# NEXORA M2 — nexora session lifecycle — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the Lavalink v4 session surface — `GET /nexora/sessions/{id}` returning the Session object, `PATCH /nexora/sessions/{id}` applying resume settings, and the nexora session state that a real client reads.

**Architecture:** The nexora session DTOs live in `crates/nexora-protocol/src/nexora/`; the resumable state lives in `nexora-session` next to the rest of the per-session record; the server maps between them. The nexora session routes stop serving a WebSocket there — that moves to `/nexora/websocket` in M1.

**Tech Stack:** Rust 2024, axum, serde, `nexora-protocol` nexora types, `nexora-session` state.

**Spec:** `docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md` (M2 row) and `docs/COMPATIBILITY.md`. Reference evidence: `docs/research/lavalink-source-architecture.md` §4.8 and the shipped `Lavalink.jar` (`SessionRestHandler.kt:13-25`).

## Global Constraints

- Rust stable, edition 2024, pinned dependencies; `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` green before the milestone closes.
- No `unwrap`, `expect` or `panic!` in production paths that can fail at runtime; errors flow through `nexora-error`.
- Third-party crates are allowed; the server, protocol, session and player logic are ours.
- Dependencies respect their licences; attribution is recorded.
- Large downloads and installs go to the E drive.
- Documentation layout follows ADR-021: new docs linked from `docs/README.md`; architecture decisions are new ADRs, never rewritten old ones.
- Every behavioural claim cites evidence: REA `ev_…`, craft-anything `ev_…`, or `spec:` for a line in `docs/research/lavalink-source-architecture.md`.

## Review Focus

1. `GET /nexora/sessions/{id}` must return the **Session object** (`resumable`, `resumeTimeout`), not a WebSocket — a Lavalink client that GETs the session expects that JSON and currently gets a socket.
2. `PATCH /nexora/sessions/{id}` with an absent `resuming` or `timeout` must leave the stored value untouched; a present value must replace it.
3. `timeout` is a duration in **milliseconds** on the wire but stored in **seconds** — a `timeout: 5000` must be stored as 5 and echoed as 5000.
4. A PATCH for a session that does not exist must be a 404, not a 500 and not a silent success.
5. The native `/api/v1` session surface must keep working unchanged — the nexora route change must not touch `/api/v1/sessions/{id}`.

---

## Reference facts this plan is built on

| Fact | Value | Source |
|---|---|---|
| `PATCH /nexora/sessions/{id}` body | `{ "resuming": <bool?>, "timeout": <duration?> }` | `spec:` §4.8 (`SessionRestHandler.kt:13-25`) |
| `PATCH /nexora/sessions/{id}` effect | `resuming` sets `context.resumable`; `timeout` sets `context.resumeTimeout` in **seconds** | `spec:` §4.8 |
| `PATCH /nexora/sessions/{id}` response | the `Session` object | `spec:` §4.8 |
| Session object | `{ "resumable": <bool>, "resumeTimeout": <duration> }` | `spec:` §4.8 |
| nexora route inventory | `PATCH /nexora/sessions/{sessionId}` is a nexora route; there is **no** `GET /nexora/sessions/{id}` in the inventory | `spec:` §4.1 |

**Note on the shape:** NEXORA currently serves `GET /nexora/sessions/{id}` as an event WebSocket (`services/nexora-server/src/lib.rs`). The reference's session object is what a nexora client reads from the session route, and the reference's event socket is `/nexora/websocket` (M1). This plan moves the session route to the reference's meaning and leaves the native `/ws/v1/sessions/{id}` socket as it is.

---

## File structure

```
crates/nexora-protocol/src/nexora/session.rs  # NEW: NexoraSession, NexoraSessionUpdate
crates/nexora-session/src/lib.rs          # MODIFIED: resumable + resume timeout in the record
services/nexora-server/src/lib.rs         # MODIFIED: GET and PATCH /nexora/sessions/{id}
services/nexora-server/tests/compat/main.rs  # MODIFIED: session lifecycle tests
docs/COMPATIBILITY.md                     # MODIFIED: the session rows
```

---

### Task 1: the nexora session DTOs

**Files:**
- Create: `crates/nexora-protocol/src/nexora/session.rs`
- Modify: `crates/nexora-protocol/src/nexora/mod.rs`, `crates/nexora-protocol/src/lib.rs`
- Test: `crates/nexora-protocol/src/nexora/session.rs` (inline tests)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct NexoraSession { pub resumable: bool, pub resume_timeout_ms: u64 }` — wire keys `resumable`, `resumeTimeout`.
  - `pub struct NexoraSessionUpdate { pub resuming: Option<bool>, pub timeout_ms: Option<u64> }` — wire keys `resuming`, `timeout`; absent means "leave unchanged".

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn session_serialises_the_reference_shape() {
    let session = NexoraSession { resumable: true, resume_timeout_ms: 60_000 };
    let value = serde_json::to_value(&session).unwrap();
    assert_eq!(value["resumable"], true);
    assert_eq!(value["resumeTimeout"], 60_000);
}

#[test]
fn session_update_distinguishes_absent_from_present() {
    let empty: NexoraSessionUpdate = serde_json::from_str("{}").unwrap();
    assert_eq!(empty.resuming, None);
    assert_eq!(empty.timeout_ms, None);
    let set: NexoraSessionUpdate =
        serde_json::from_str(r#"{"resuming": true, "timeout": 5000}"#).unwrap();
    assert_eq!(set.resuming, Some(true));
    assert_eq!(set.timeout_ms, Some(5000));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p nexora-protocol nexora::session`
Expected: FAIL with "cannot find type `NexoraSession`".

- [ ] **Step 3: Implement `nexora/session.rs`**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraSession {
    pub resumable: bool,
    #[serde(rename = "resumeTimeout")]
    pub resume_timeout_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraSessionUpdate {
    #[serde(default)]
    pub resuming: Option<bool>,
    #[serde(default, rename = "timeout")]
    pub timeout_ms: Option<u64>,
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p nexora-protocol nexora::session`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/nexora-protocol/src/nexora/session.rs crates/nexora-protocol/src/nexora/mod.rs crates/nexora-protocol/src/lib.rs
git commit -m "feat(protocol): add nexora session DTOs"
```

---

### Task 2: resumable session state

**Files:**
- Modify: `crates/nexora-session/src/lib.rs`
- Test: `crates/nexora-session/tests/session.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct SessionResumeSettings { pub resumable: bool, pub timeout_seconds: u64 }`
  - `pub async fn resume_settings(&self, session_id: &SessionId) -> NexoraResult<SessionResumeSettings>`
  - `pub async fn set_resumable(&self, session_id: &SessionId, resumable: bool) -> NexoraResult<()>`
  - `pub async fn set_resume_timeout(&self, session_id: &SessionId, timeout_seconds: u64) -> NexoraResult<()>`

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn resume_settings_start_insecure_and_can_be_updated() {
    let manager = SessionManager::new();
    let session_id = manager.create(AudioFormat::new(48_000, 2, AudioCodec::Opus)).await.unwrap().session_id;
    let settings = manager.resume_settings(&session_id).await.unwrap();
    assert!(!settings.resumable);
    manager.set_resumable(&session_id, true).await.unwrap();
    manager.set_resume_timeout(&session_id, 60).await.unwrap();
    let updated = manager.resume_settings(&session_id).await.unwrap();
    assert!(updated.resumable);
    assert_eq!(updated.timeout_seconds, 60);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p nexora-session resume_settings`
Expected: FAIL with "no method named `resume_settings`".

- [ ] **Step 3: Implement the state**

Add `resumable: bool` and `resume_timeout_seconds: u64` to `SessionRecord`, defaulting to `false` and `60` (a minute is the reference's default resume window), and implement the three methods. `set_resumable` and `set_resume_timeout` must return `NexoraError::service("session not found")` for an unknown session — the same error the rest of the manager uses.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p nexora-session resume_settings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/nexora-session/src/lib.rs crates/nexora-session/tests/session.rs
git commit -m "feat(session): track resumable session settings"
```

---

### Task 3: the nexora session routes

**Files:**
- Modify: `services/nexora-server/src/lib.rs`
- Test: `services/nexora-server/tests/compat/main.rs`

**Interfaces:**
- Consumes: `NexoraSession`, `NexoraSessionUpdate` from Task 1; the session methods from Task 2.
- Produces:
  - `GET /nexora/sessions/{id}` → the `NexoraSession` object as JSON.
  - `PATCH /nexora/sessions/{id}` → applies `resuming` and `timeout`, then returns the `NexoraSession` object.

- [ ] **Step 1: Write the failing tests**

```rust
/// Row: `/nexora/sessions/{id}` — the session object.
#[tokio::test]
async fn compat_nexora_session_endpoint_returns_the_session_object() {
    // create a session, GET it, assert { resumable, resumeTimeout }
}

/// Row: `PATCH /nexora/sessions/{id}` — resume settings round-trip.
#[tokio::test]
async fn compat_nexora_session_patch_applies_resume_settings() {
    // create a session, PATCH { "resuming": true, "timeout": 5000 },
    // assert the response has resumable=true and resumeTimeout=5000
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p nexora-server --test compat compat_nexora_session`
Expected: FAIL — the route returns a WebSocket upgrade, not the session object.

- [ ] **Step 3: Implement the routes**

Replace the `GET /nexora/sessions/{id}` route with the session object and add the PATCH:

```rust
async fn lavalink_session(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, ApiError> {
    let session_id = parse_session_id(&id)?;
    let settings = state.sessions.resume_settings(&session_id).await?;
    Ok(Json(json!({
        "resumable": settings.resumable,
        "resumeTimeout": settings.timeout_seconds * 1000,
    })))
}

async fn lavalink_patch_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(update): Json<NexoraSessionUpdate>,
) -> Result<Json<Value>, ApiError> {
    let session_id = parse_session_id(&id)?;
    if let Some(resuming) = update.resuming {
        state.sessions.set_resumable(&session_id, resuming).await?;
    }
    if let Some(timeout_ms) = update.timeout_ms {
        state.sessions.set_resume_timeout(&session_id, timeout_ms / 1000).await?;
    }
    // then return the same object as GET
}
```

The event WebSocket that used to live on `GET /nexora/sessions/{id}` moves to `/nexora/websocket` in M1 — until M1 lands, keep the existing per-session socket at `/ws/v1/sessions/{id}` (it already exists) and make sure the native surface is unchanged.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p nexora-server`
Expected: PASS — including the pre-existing `lavalink_player_matches_documented_nexora_shape` test, which uses `/nexora/sessions/{id}/player` and must not change.

- [ ] **Step 5: Commit**

```bash
git add services/nexora-server/src/lib.rs services/nexora-server/tests/compat/main.rs
git commit -m "feat(server): serve and patch the nexora session resource"
```

---

### Task 4: matrix and milestone close

**Files:**
- Modify: `docs/COMPATIBILITY.md`, `docs/PROJECT_STATUS.md`
- Test: none (documentation)

**Interfaces:**
- Consumes: the proofs from Tasks 1-3.
- Produces: the session rows in the matrix carry their proof names.

- [ ] **Step 1: Update the matrix rows**

`PATCH /nexora/sessions/{id}` moves from `missing` to `implemented` with proof `compat_nexora_session_patch_applies_resume_settings`. Add a row for the session object with proof `compat_nexora_session_endpoint_returns_the_session_object`.

- [ ] **Step 2: Run the whole workspace**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: all three exit 0; state the exact test count in the commit.

- [ ] **Step 3: Commit**

```bash
git add docs/COMPATIBILITY.md docs/PROJECT_STATUS.md
git commit -m "docs: record the M2 nexora session milestone"
```

---

## Self-review

**Spec coverage.** The M2 row names create (exists), `loadTracks` (exists), play/pause/stop/seek/volume (exist on the native surface; the nexora player routes are M7), and `PATCH /nexora/sessions/{id}` (Task 3). The exit proof "real client play/pause/resume; matrix rows green" is covered by the compat tests plus the matrix update.

**Step scan.** Every step names a file, a signature or a test name, and a command with an expected result.

**Type consistency.** `NexoraSession` and `NexoraSessionUpdate` are defined in Task 1 and used by name in Task 3; `resume_settings`, `set_resumable` and `set_resume_timeout` are defined in Task 2 and used by the same names in Task 3.

**Review Focus.** All five lines have an owning task: the session object replacing the socket (Task 3), absent values leaving state untouched (Task 3), the millisecond/second conversion (Tasks 2 and 3), 404 for an unknown session (Task 2's error contract, Task 3's handler), and the native surface staying unchanged (Task 3's test run covers the pre-existing suite).

**Proportion.** Four tasks, four test blocks; no implementation bodies beyond the values the reference fixes.
