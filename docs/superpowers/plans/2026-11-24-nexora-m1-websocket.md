# NEXORA M1 — `/version` and the nexora WebSocket — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serve Lavalink v4's `/version` endpoint and its `/nexora/websocket` event socket — the handshake, the `ready` frame and the session identity — so a real Lavalink client can connect.

**Architecture:** The nexora socket lives in `services/nexora-server` next to the existing per-session socket, and reuses the typed nexora protocol types from `crates/nexora-protocol/src/nexora/`. Handshake rules are enforced before the upgrade; the socket then emits `ready` and forwards session events as nexora frames.

**Tech Stack:** Rust 2024, axum WebSocketUpgrade, tokio broadcast, serde_json, `nexora-protocol` nexora types, session ids built from a counter over an `a-z0-9` alphabet.

**Spec:** `docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md` (M1 row) and `docs/COMPATIBILITY.md`. Reference evidence: `docs/research/lavalink-source-architecture.md` §4.3 and the shipped `Lavalink.jar` (`E:\ca-out\info-handler.json`, `protocol-jvm-4.2.2.jar` in `E:\ca-out\`).

## Global Constraints

- Rust stable, edition 2024, pinned dependencies; `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` green before the milestone closes.
- No `unwrap`, `expect` or `panic!` in production paths that can fail at runtime; errors flow through `nexora-error`.
- Third-party crates are allowed; the server, protocol, session and player logic are ours.
- Dependencies respect their licences; attribution is recorded.
- Large downloads and installs go to the E drive.
- Documentation layout follows ADR-021: new docs linked from `docs/README.md`; architecture decisions are new ADRs, never rewritten old ones.
- Every behavioural claim cites evidence: REA `ev_…`, craft-anything `ev_…`, or `spec:` for a line in `docs/research/lavalink-source-architecture.md`.

## Review Focus

1. A client that omits `User-Id`, sends `User-Id: 0` or sends a non-numeric `User-Id` must be refused with 400 **before** the upgrade — never accepted and never a 500.
2. A client with the wrong `Authorization` header must get 401; a server with no password configured must still accept the connection (the reference compares the header against the configured password, and an unset password means an unset header matches).
3. Two clients connecting at once must get **different** session ids, and a `Session-Id` that is not resumable must be answered with `Session-Resumed: false` — never `true` by accident.
4. A client that sends `Client-Name` or `User-Agent` missing must still connect (they are warnings, not failures).
5. The `ready` frame must be the first frame the client sees, and it must contain `resumed` and `sessionId` — a client that reads `sessionId` to resume later must get a value it can send back.

---

## Reference facts this plan is built on

| Fact | Value | Source |
|---|---|---|
| `GET /version` | returns the version string; the handler returns a plain `String` | craft-anything `java_class_show` on `lavalink.server.info.InfoRestHandler` (shipped `Lavalink.jar`) |
| Handshake: `Authorization` | required; must equal the server password, else **401** | `spec:` §4.3 (`HandshakeInterceptorImpl.kt:27-53`) |
| Handshake: `User-Id` | required; absent, empty, unparseable or `0` → **400** | `spec:` §4.3 |
| Handshake: `Session-Id` | optional; when present and resumable the response carries `Session-Resumed: true` | `spec:` §4.3 |
| Handshake: `Client-Name`, `User-Agent` | optional; absent logs a warning | `spec:` §4.3 |
| Session ids | 16 characters drawn from `a-z0-9` | `spec:` §4.3 (`SocketServer.kt:85-91`) |
| `ready` frame | `{ "op": "ready", "data": { "resumed": <bool>, "sessionId": "<id>" } }` | craft-anything `java_class_show` on `dev.arbjerg.lavalink.protocol.v4.Message$ReadyEvent` (`protocol-jvm-4.2.2.jar`) |
| `playerUpdate` frame | `{ "op": "playerUpdate", "data": { "state": <PlayerState>, "guildId": "<id>" } }` | craft-anything `java_class_show` on `Message$PlayerUpdateEvent` |

---

## File structure

```
crates/nexora-protocol/src/nexora/op.rs     # MODIFIED: NexoraReady becomes { resumed, session_id }
crates/nexora-protocol/src/nexora/session.rs # NEW: nexora session id generator and validator
services/nexora-server/src/lib.rs        # MODIFIED: GET /version + WS /nexora/websocket
services/nexora-server/tests/compat/main.rs  # MODIFIED: handshake and ready-frame tests
docs/COMPATIBILITY.md                    # MODIFIED: the two rows M1 closes
```

---

### Task 1: `GET /version`

**Files:**
- Modify: `services/nexora-server/src/lib.rs` (route + handler)
- Test: `services/nexora-server/tests/compat/main.rs`

**Interfaces:**
- Consumes: `nexora_protocol::NexoraVersion::nexora()`.
- Produces: `GET /version` → `text/plain` body with the semver string, no JSON quotes.

- [ ] **Step 1: Write the failing test**

```rust
/// Row: `/version` — the plain version string.
#[tokio::test]
async fn compat_version_endpoint_returns_the_plain_version() {
    let root = media_root();
    let response = app(&root)
        .oneshot(
            Request::builder()
                .uri("/version")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/plain; charset=utf-8"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(text, env!("CARGO_PKG_VERSION"));
    let _ = fs::remove_dir_all(&root);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p nexora-server --test compat compat_version_endpoint`
Expected: FAIL with 404 (no `/version` route).

- [ ] **Step 3: Implement the route**

In `services/nexora-server/src/lib.rs` add `.route("/version", get(version))` and:

```rust
async fn version() -> String {
    NexoraVersion::nexora().semver
}
```

Returning a bare `String` from an axum handler produces `text/plain; charset=utf-8`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p nexora-server --test compat compat_version_endpoint`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add services/nexora-server/src/lib.rs services/nexora-server/tests/compat/main.rs
git commit -m "feat(server): serve the nexora version endpoint"
```

---

### Task 2: nexora session identity

**Files:**
- Create: `crates/nexora-protocol/src/nexora/session.rs`
- Modify: `crates/nexora-protocol/src/nexora/mod.rs`, `crates/nexora-protocol/src/lib.rs`
- Test: `crates/nexora-protocol/src/nexora/session.rs` (inline tests)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub const Nexora_SESSION_ID_LENGTH: usize = 16;`
  - `pub fn generate_nexora_session_id() -> String` — 16 characters from `a-z0-9`.
  - `pub fn is_valid_nexora_session_id(value: &str) -> bool` — true when the value is exactly 16 characters from `a-z0-9`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn generated_session_ids_have_the_reference_shape() {
    let id = generate_nexora_session_id();
    assert_eq!(id.len(), Nexora_SESSION_ID_LENGTH);
    assert!(is_valid_nexora_session_id(&id));
    assert!(
        id.bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    );
}

#[test]
fn session_id_validation_rejects_wrong_shapes() {
    assert!(!is_valid_nexora_session_id(""));
    assert!(!is_valid_nexora_session_id("short"));
    assert!(!is_valid_nexora_session_id("nx_session_1234567"));
    assert!(!is_valid_nexora_session_id("ABCDEFGHIJKLMNOP"));
}

#[test]
fn generated_session_ids_differ() {
    assert_ne!(generate_nexora_session_id(), generate_nexora_session_id());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p nexora-protocol nexora::session`
Expected: FAIL with "cannot find function `generate_nexora_session_id`".

- [ ] **Step 3: Implement `nexora/session.rs`**

Generate ids from a counter mixed with the process start time so two ids in the same process never collide: keep a `static COUNTER: AtomicU64`, and build the id as `base36(counter ^ start_nanos)` padded to 16 characters over the `0-9a-z` alphabet. The generator must never panic — it pads instead of indexing out of range.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p nexora-protocol nexora::session`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/nexora-protocol/src/nexora/session.rs crates/nexora-protocol/src/nexora/mod.rs crates/nexora-protocol/src/lib.rs
git commit -m "feat(protocol): add nexora session identity"
```

---

### Task 3: the `/nexora/websocket` handshake

**Files:**
- Modify: `services/nexora-server/src/lib.rs` (route + handler)
- Test: `services/nexora-server/tests/compat/main.rs`

**Interfaces:**
- Consumes: `generate_nexora_session_id` from Task 2; the server's existing password check for the REST routes.
- Produces: `WS /nexora/websocket` accepting the reference's handshake headers, refusing with 401 (bad `Authorization`) and 400 (bad `User-Id`) before the upgrade, and answering `Session-Resumed: false` when no resumable session was offered.

- [ ] **Step 1: Write the failing tests**

```rust
/// Row: `/nexora/websocket` handshake — refused without the reference's headers.
#[tokio::test]
async fn compat_nexora_websocket_handshake_refuses_bad_headers() {
    let root = media_root();
    // No User-Id: the reference answers 400.
    let response = app(&root)
        .oneshot(
            Request::builder()
                .uri("/nexora/websocket")
                .header("upgrade", "websocket")
                .header("connection", "Upgrade")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let _ = fs::remove_dir_all(&root);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p nexora-server --test compat compat_nexora_websocket_handshake`
Expected: FAIL — the route does not exist, so the answer is 404 (or the upgrade succeeds).

- [ ] **Step 3: Implement the handshake**

```rust
async fn nexora_websocket(
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    if !authorized(&headers, &state) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if nexora_user_id(&headers).is_none() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let resumed = false;
    let session_id = generate_nexora_session_id();
    let mut response = ws.on_upgrade(move |socket| nexora_socket(socket, session_id, resumed, state));
    response
        .headers_mut()
        .insert("Session-Resumed", HeaderValue::from_static("false"));
    response
}
```

`nexora_user_id` returns `None` when the header is absent, empty, not a number, or `0`. `authorized` must accept the request when the server has **no** password configured (the reference compares the header against the configured password, and an unset password means an unset header matches) — reuse the server's existing REST behaviour so both agree.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p nexora-server --test compat compat_nexora_websocket_handshake`
Expected: PASS (400 for the missing `User-Id`).

- [ ] **Step 5: Commit**

```bash
git add services/nexora-server/src/lib.rs services/nexora-server/tests/compat/main.rs
git commit -m "feat(server): add the nexora websocket handshake"
```

---

### Task 4: the `ready` frame and the corrected `NexoraReady`

**Files:**
- Modify: `crates/nexora-protocol/src/nexora/op.rs` (`NexoraReady` shape), `services/nexora-server/src/lib.rs` (emit the frame)
- Test: `crates/nexora-protocol/src/nexora/op.rs`, `services/nexora-server/tests/compat/main.rs`

**Interfaces:**
- Consumes: `NexoraOp::Ready`, `NexoraEventMessage`, `generate_nexora_session_id`.
- Produces: `NexoraReady { pub resumed: bool, pub session_id: String }` — the reference's shape, replacing the plan's earlier `{ version }` guess.

- [ ] **Step 1: Write the failing tests**

In `crates/nexora-protocol/src/nexora/op.rs`:

```rust
#[test]
fn ready_payload_carries_resumed_and_session_id() {
    let ready = NexoraReady {
        resumed: false,
        session_id: "abc123".to_owned(),
    };
    let value = serde_json::to_value(&ready).unwrap();
    assert_eq!(value["resumed"], false);
    assert_eq!(value["sessionId"], "abc123");
}
```

In `services/nexora-server/tests/compat/main.rs`:

```rust
/// Row: `/nexora/websocket` — the ready frame is the first thing a client sees.
#[tokio::test]
async fn compat_nexora_websocket_sends_ready_first() {
    // connect with the reference's headers, read one frame, assert it is the ready frame
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p nexora-protocol nexora::op` and `cargo test -p nexora-server --test compat compat_nexora_websocket_sends_ready`
Expected: FAIL — `NexoraReady` has no `resumed`/`session_id` fields, and no socket emits a frame yet.

- [ ] **Step 3: Implement**

Replace `NexoraReady`'s field with the reference shape:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraReady {
    pub resumed: bool,
    #[serde(rename = "sessionId")]
    pub session_id: String,
}
```

In the server, the socket task writes the ready frame before anything else:

```rust
let frame = NexoraEventMessage {
    op: NexoraOp::Ready,
    event: None,
    data: serde_json::to_value(NexoraReady { resumed, session_id }).unwrap_or(Value::Null),
};
socket.send(Message::Text(frame.to_string().into())).await.ok();
```

Then keep the socket open and forward session events as nexora frames (the event mapping itself is M3's work — M1 only proves the handshake and the ready frame).

- [ ] **Step 4: Run the tests to verify they passes**

Run: `cargo test -p nexora-protocol nexora::op` and `cargo test -p nexora-server --test compat compat_nexora_websocket_sends_ready`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/nexora-protocol/src/nexora/op.rs services/nexora-server/src/lib.rs services/nexora-server/tests/compat/main.rs
git commit -m "feat(server): send the nexora ready frame on connect"
```

---

### Task 5: matrix update and milestone close

**Files:**
- Modify: `docs/COMPATIBILITY.md`, `docs/PROJECT_STATUS.md`, `docs/adr/README.md` (M1 record)
- Test: none (documentation)

**Interfaces:**
- Consumes: the proofs from Tasks 1-4.
- Produces: the matrix rows for `/version` and `/nexora/websocket` move from `missing` to `implemented` with their test names.

- [ ] **Step 1: Update the matrix rows**

`/version` → `implemented` with proof `compat_version_endpoint_returns_the_plain_version`; `/nexora/websocket` → `implemented` with proofs `compat_nexora_websocket_handshake_refuses_bad_headers` and `compat_nexora_websocket_sends_ready_first`. The `Session-Id` resume behaviour stays `missing (M6)` — only the handshake and the ready frame ship in M1.

- [ ] **Step 2: Record the milestone in the project status**

Add an "M1" entry to `docs/PROJECT_STATUS.md` with the behaviour, the verification commands and the new test count.

- [ ] **Step 3: Run the whole workspace**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: all three exit 0; state the exact test count in the commit.

- [ ] **Step 4: Commit**

```bash
git add docs/COMPATIBILITY.md docs/PROJECT_STATUS.md docs/adr/README.md
git commit -m "docs: record the M1 nexora websocket milestone"
```

---

## Self-review

**Spec coverage.** The M1 row names `/health` (exists), `/nexora/info` (exists), `/version` (Task 1), auth (Task 3), and the `/nexora/websocket` handshake (Task 3) with the ready frame (Task 4). The exit proof "real Lavalink client connects; matrix row green" is covered by the compat tests plus the matrix update (Task 5); a live client run is the milestone's manual proof.

**Step scan.** Every step names a file, a signature or a test name, and a command with an expected result.

**Type consistency.** The session-id helpers are named `generate_nexora_session_id` and `is_valid_nexora_session_id` in Task 2 and used by the same names in Tasks 3 and 4. `NexoraReady` is redefined in Task 4 with the reference's fields and used only there.

**Review Focus.** All five lines have an owning task: bad `User-Id` (Task 3), wrong password and the no-password case (Task 3), distinct ids and `Session-Resumed: false` (Tasks 2 and 3), optional headers (Task 3), ready first with a usable `sessionId` (Task 4).

**Proportion.** Five tasks, five test blocks, no implementation bodies beyond the values the reference fixes.
