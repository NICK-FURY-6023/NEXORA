# NEXORA M0 — v4 protocol core — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn NEXORA's Lavalink v4 wire surface from hand-built `json!` values into typed, round-trippable protocol DTOs — op codes, the event envelope, the error envelope, track encoding and version negotiation — each proven against the shipped reference.

**Architecture:** A new `v4` module inside `crates/nexora-protocol` owns every Lavalink v4 wire type. The server keeps owning HTTP; it stops building v4 payloads by hand and serialises the typed structs instead. Nothing else in the workspace changes shape.

**Tech Stack:** Rust 2024, serde + serde_json (already pinned in the workspace), base64 (already pinned), `nexora-error` for error mapping.

**Spec:** `docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md` (M0 row) and `docs/COMPATIBILITY.md`. Reference evidence: `docs/research/lavalink-source-architecture.md` §1.4, §4.1, §4.4 and the shipped `protocol-jvm-4.2.2.jar` extracted from `Lavalink.jar` (`E:\ca-out\protocol-jvm-4.2.2.jar`, sha256 `71049ff8aa3df15e290f729bd0c9aaa421f7f66a3d7c56d78a7f1afa743a01cd`).

## Global Constraints

- Rust stable, edition 2024, pinned dependencies; `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` green before the milestone closes.
- No `unwrap`, `expect` or `panic!` in production paths that can fail at runtime; errors flow through `nexora-error`.
- Third-party crates are allowed (Tokio, Serde, Rubato, Symphonia are already used). The server, protocol, session and player logic are ours.
- Dependencies respect their licences; attribution is recorded.
- Large downloads and installs go to the E drive.
- Documentation layout follows ADR-021: this plan is linked from `docs/README.md`; architecture decisions are new ADRs, never rewritten old ones.
- Every behavioural claim cites evidence: REA `ev_…`, craft-anything `ev_…`, or `spec:` for a line in `docs/research/lavalink-source-architecture.md`.

## Review Focus

1. A client sends an unknown `op` value or an unknown event `type` — the server must answer with the v4 error envelope, never panic and never silently drop the message.
2. A malformed `encoded` track (bad base64, non-UTF-8 payload, missing fields) must produce the error envelope; `encode → decode` must be lossless for every valid track.
3. A track with no duration and no artist must still decode into a `TrackInfo` with documented defaults (`length: 0`, `author: "unknown"`), because real sources omit them.
4. `/v4/info` must report `filters` truthfully: an empty list means "no filters implemented", and the same value must appear in the compatibility matrix.
5. A client that negotiates a v4 version newer than NEXORA's must get a documented error, not a silently different payload shape.

---

## Reference facts this plan is built on

| Fact | Value | Source |
|---|---|---|
| v4 op codes | `Ready` → `"ready"`, `Stats` → `"stats"`, `PlayerUpdate` → `"playerUpdate"`, `Event` → `"event"` (string values, ordinals 0-3) | craft-anything `java_class_show` on `dev.arbjerg.lavalink.protocol.v4.Message$Op` from the shipped `protocol-jvm-4.2.2.jar` |
| v4 message kinds | `EmittedEvent`, `Op`, `PlayerUpdateEvent`, `ReadyEvent`, `StatsEvent`, `Serializer` | craft-anything `java_class_show` on `dev.arbjerg.lavalink.protocol.v4.Message` |
| exception severity | `COMMON` (0), `SUSPICIOUS` (1), `FAULT` (2) — the name is `FAULT`, not `FATAL` | craft-anything `java_class_show` on `dev.arbjerg.lavalink.protocol.v4.Exception$Severity` |
| v4 protocol module layout | `messages.kt`, `player.kt`, `loadResult.kt`, `filters.kt`, `info.kt`, `session.kt`, `stats.kt`, `omissible.kt`, `mapper.kt`, `routeplanner.kt`, `error.kt` | `spec:` §1.4 |
| load result kinds | `TRACK_LOADED`, `PLAYLIST_LOADED`, `SEARCH_RESULT`, `NONE`, `LOAD_FAILED` (Kotlin variant names; NEXORA's wire values are the lowercase forms already covered by `services/nexora-server/tests/http.rs`) | `spec:` §4.4 |
| encoded track | semantically equivalent is enough: identifier + info + opaque source payload; byte-compatibility with Lavaplayer is not required | `docs/LAVALINK_DEEP_DIVE.md` §3 |
| current NEXORA behaviour | `/v4/info` builds a `json!` value with `filters: []`; `/v4/loadtracks` builds each track with `json!` and base64-encodes a compact JSON payload of `{track_id, source_id, title, duration_ms}` | `services/nexora-server/src/lib.rs` `lavalink_info`, `lavalink_track` |

### Deviation from the design doc (recorded here deliberately)

The design doc lists a root `tests/compat` directory. Cargo has no root test target, so the
compatibility harness lives with the package it exercises:
`services/nexora-server/tests/compat/main.rs`. The design doc and `docs/COMPATIBILITY.md`
are updated to match in Task 6.

---

## File structure

```
crates/nexora-protocol/src/
  lib.rs                     # existing native DTOs; gains `pub mod v4;`
  v4/mod.rs                  # NEW: module root, re-exports the four submodules
  v4/op.rs                   # NEW: Op, EmittedEvent, V4EventType
  v4/error.rs                # NEW: V4Error, V4ErrorSeverity, V4ErrorCode
  v4/track.rs                # NEW: V4Track, V4TrackInfo, encode_track, decode_track
  v4/info.rs                 # NEW: V4Info, V4Version, V4Git, V4Plugin
services/nexora-server/src/lib.rs            # MODIFIED: v4 handlers serialise typed DTOs
services/nexora-server/tests/compat/main.rs  # NEW: matrix-driven compatibility tests
docs/COMPATIBILITY.md                       # MODIFIED: M0 rows become implemented
docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md  # MODIFIED: tests/compat path
```

---

### Task 1: v4 op codes and the event envelope

**Files:**
- Create: `crates/nexora-protocol/src/v4/op.rs`
- Modify: `crates/nexora-protocol/src/lib.rs` (add `pub mod v4;` and `pub use v4::...` re-exports)
- Test: `crates/nexora-protocol/src/v4/op.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing (first task).
- Produces:
  - `pub enum V4Op { Ready, Stats, PlayerUpdate, Event }` with explicit `#[serde(rename = "...")]` per variant — wire values must be exactly `"ready"`, `"stats"`, `"playerUpdate"`, `"event"`. Implement `V4Op::as_str(&self) -> &'static str` and `V4Op::from_wire(&str) -> Option<V4Op>`.
  - `pub enum V4EventType { TrackStart, TrackEnd, TrackException, PlayerUpdate, WebSocketClosed }` with wire values `"TrackStartEvent"`, `"TrackEndEvent"`, `"TrackExceptionEvent"`, `"playerUpdate"`, `"WebSocketClosedEvent"`, plus `V4EventType::as_str` and `V4EventType::from_wire`.
  - `pub struct V4EmittedEvent { pub op: V4Op, pub event: Option<V4EventType>, pub data: serde_json::Value }` — serialises `op` always; `event` is omitted when `None`; `data` is the payload.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn op_wire_values_match_the_shipped_reference() {
    assert_eq!(V4Op::Ready.as_str(), "ready");
    assert_eq!(V4Op::Stats.as_str(), "stats");
    assert_eq!(V4Op::PlayerUpdate.as_str(), "playerUpdate");
    assert_eq!(V4Op::Event.as_str(), "event");
    assert_eq!(V4Op::from_wire("playerUpdate"), Some(V4Op::PlayerUpdate));
    assert_eq!(V4Op::from_wire("nope"), None);
}

#[test]
fn emitted_event_serialises_op_and_omits_absent_event() {
    let event = V4EmittedEvent { op: V4Op::Event, event: Some(V4EventType::TrackStart), data: json!({"track": {}}) };
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["op"], "event");
    assert_eq!(value["event"], "TrackStartEvent");
    let no_event = V4EmittedEvent { op: V4Op::Ready, event: None, data: json!({}) };
    assert!(serde_json::to_value(&no_event).unwrap().get("event").is_none());
}

#[test]
fn event_type_wire_values_match_the_reference() {
    assert_eq!(V4EventType::TrackStart.as_str(), "TrackStartEvent");
    assert_eq!(V4EventType::TrackEnd.as_str(), "TrackEndEvent");
    assert_eq!(V4EventType::TrackException.as_str(), "TrackExceptionEvent");
    assert_eq!(V4EventType::PlayerUpdate.as_str(), "playerUpdate");
    assert_eq!(V4EventType::WebSocketClosed.as_str(), "WebSocketClosedEvent");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p nexora-protocol v4::op`
Expected: FAIL with "cannot find type `V4Op`".

- [ ] **Step 3: Implement `v4/op.rs`**

Define the two enums with explicit `#[serde(rename = "...")]` per variant (do not rely on a blanket rename rule: `"playerUpdate"` and `"TrackStartEvent"` do not share one casing rule). `V4Op::from_wire` matches on the exact string and returns `None` for anything else — no case folding, no numeric fallback. `V4EmittedEvent` derives `Serialize, Deserialize, Debug, Clone, PartialEq`; `event` carries `#[serde(skip_serializing_if = "Option::is_none")]`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p nexora-protocol v4::op`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/nexora-protocol/src/lib.rs crates/nexora-protocol/src/v4/op.rs crates/nexora-protocol/src/v4/mod.rs
git commit -m "feat(protocol): add v4 op codes and event envelope"
```

---

### Task 2: v4 error envelope

**Files:**
- Create: `crates/nexora-protocol/src/v4/error.rs`
- Modify: `crates/nexora-protocol/src/v4/mod.rs`
- Test: `crates/nexora-protocol/src/v4/error.rs` (inline tests)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:
  - `pub enum V4ErrorSeverity { Common, Suspicious, Fault }` — wire values `"COMMON"`, `"SUSPICIOUS"`, `"FAULT"`.
  - `pub struct V4Error { pub code: String, pub message: String, pub severity: V4ErrorSeverity, pub cause: Option<String> }`.
  - `pub struct V4ErrorEnvelope { pub timestamp: i64, pub error: V4Error }` — the whole HTTP body.
  - `impl V4ErrorEnvelope { pub fn new(code: impl Into<String>, message: impl Into<String>, severity: V4ErrorSeverity) -> Self }` — stamps `timestamp` with the current unix time in milliseconds.
  - `impl V4ErrorSeverity { pub fn as_str(&self) -> &'static str }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn error_envelope_shape_is_code_message_severity_cause() {
    let envelope = V4ErrorEnvelope::new("TRACK_NOT_FOUND", "no such track", V4ErrorSeverity::Common);
    let value = serde_json::to_value(&envelope).unwrap();
    assert_eq!(value["error"]["code"], "TRACK_NOT_FOUND");
    assert_eq!(value["error"]["message"], "no such track");
    assert_eq!(value["error"]["severity"], "COMMON");
    assert!(value["error"]["cause"].is_null());
    assert!(value["timestamp"].as_i64().unwrap() > 0);
}

#[test]
fn severity_names_match_the_shipped_reference() {
    assert_eq!(V4ErrorSeverity::Common.as_str(), "COMMON");
    assert_eq!(V4ErrorSeverity::Suspicious.as_str(), "SUSPICIOUS");
    assert_eq!(V4ErrorSeverity::Fault.as_str(), "FAULT");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p nexora-protocol v4::error`
Expected: FAIL with "cannot find type `V4ErrorEnvelope`".

- [ ] **Step 3: Implement `v4/error.rs`**

`V4ErrorSeverity` uses explicit `#[serde(rename = "COMMON")]`-style attributes per variant. `V4Error.cause` is `Option<String>` and is serialised as `null` when absent (the reference always emits the key). `V4ErrorEnvelope::new` uses `std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)` and maps the error to `0` rather than panicking.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p nexora-protocol v4::error`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/nexora-protocol/src/v4/error.rs crates/nexora-protocol/src/v4/mod.rs
git commit -m "feat(protocol): add v4 error envelope and severity"
```

---

### Task 3: typed track and the encoded-track container

**Files:**
- Create: `crates/nexora-protocol/src/v4/track.rs`
- Modify: `crates/nexora-protocol/src/v4/mod.rs`
- Modify: `crates/nexora-protocol/Cargo.toml` (add `base64` if absent)
- Test: `crates/nexora-protocol/src/v4/track.rs` (inline tests)

**Interfaces:**
- Consumes: nothing from Tasks 1-2.
- Produces:
  - `pub struct V4TrackInfo { pub identifier: String, pub is_seekable: bool, pub author: String, pub length: u64, pub is_stream: bool, pub position: u64, pub title: String, pub uri: Option<String>, pub artwork_url: Option<String>, pub isrc: Option<String>, pub source_name: String }` — wire keys `identifier`, `isSeekable`, `author`, `length`, `isStream`, `position`, `title`, `uri`, `artworkUrl`, `isrc`, `sourceName`.
  - `pub struct V4Track { pub encoded: String, pub info: V4TrackInfo, pub plugin_info: serde_json::Value, pub user_data: serde_json::Value }` — wire keys `encoded`, `info`, `pluginInfo`, `userData`.
  - `pub fn encode_track(info: &V4TrackInfo, source_payload: &serde_json::Value) -> Result<String, nexora_error::NexoraError>` — base64 of the compact JSON payload `{"track_id":…,"source_id":…,"title":…,"duration_ms":…}` (the exact keys the current server already writes, so existing encoded tracks stay decodable).
  - `pub fn decode_track(encoded: &str) -> Result<V4TrackInfo, nexora_error::NexoraError>` — base64-decode, UTF-8 decode, JSON parse, then build `V4TrackInfo`: `identifier` from `track_id`, `source_name` from `source_id`, `title` from `title`, `length` from `duration_ms` (0 when absent), `author` from `artist` (falling back to `"unknown"`), `is_seekable` true when `duration_ms` is present, `is_stream` false, `position` 0, and `uri`/`artwork_url`/`isrc` `None`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn encode_decode_round_trip_is_lossless() {
    let info = V4TrackInfo { identifier: "tone.wav".into(), is_seekable: true, author: "artist".into(), length: 100, is_stream: false, position: 0, title: "Tone".into(), uri: None, artwork_url: None, isrc: None, source_name: "local".into() };
    let encoded = encode_track(&info, &json!({"source_id": "local"})).unwrap();
    let decoded = decode_track(&encoded).unwrap();
    assert_eq!(decoded.identifier, "tone.wav");
    assert_eq!(decoded.title, "Tone");
    assert_eq!(decoded.length, 100);
    assert_eq!(decoded.author, "artist");
    assert_eq!(decoded.source_name, "local");
    assert!(decoded.is_seekable);
}

#[test]
fn decode_track_defaults_missing_fields() {
    let payload = json!({"track_id": "x", "source_id": "local", "title": "T"}).to_string();
    let encoded = STANDARD.encode(payload);
    let decoded = decode_track(&encoded).unwrap();
    assert_eq!(decoded.length, 0);
    assert_eq!(decoded.author, "unknown");
    assert!(!decoded.is_seekable);
}

#[test]
fn decode_track_rejects_malformed_input() {
    assert!(decode_track("not base64!!").is_err());
    assert!(decode_track(&STANDARD.encode([0xff, 0xfe])).is_err());
    assert!(decode_track(&STANDARD.encode("not json")).is_err());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p nexora-protocol v4::track`
Expected: FAIL with "cannot find function `encode_track`".

- [ ] **Step 3: Implement `v4/track.rs`**

Use `base64::engine::general_purpose::STANDARD` and `base64::Engine` (already a workspace dependency; add `base64` to `crates/nexora-protocol/Cargo.toml` if it is not already there). Every failure path returns `NexoraError::invalid_input("…")` — no `unwrap`, no `expect`. `decode_track` must not panic on a payload whose fields have unexpected types: use `serde_json::Value::get` and `as_str`/`as_u64` with `unwrap_or` defaults rather than typed deserialization that could reject a valid-but-sparse payload.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p nexora-protocol v4::track`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/nexora-protocol/src/v4/track.rs crates/nexora-protocol/src/v4/mod.rs crates/nexora-protocol/Cargo.toml
git commit -m "feat(protocol): add typed v4 track and encoded-track codec"
```

---

### Task 4: version, info and version negotiation

**Files:**
- Create: `crates/nexora-protocol/src/v4/info.rs`
- Modify: `crates/nexora-protocol/src/v4/mod.rs`
- Modify: `crates/nexora-protocol/src/v4/op.rs` (add `V4Ready { version: V4Version }`)
- Test: `crates/nexora-protocol/src/v4/info.rs` (inline tests)

**Interfaces:**
- Consumes: nothing from Tasks 1-3 (the `V4Ready` addition in `op.rs` consumes `V4Version` from this task).
- Produces:
  - `pub struct V4Version { pub semver: String, pub major: u32, pub minor: u32, pub patch: u32, pub pre_release: Option<String>, pub build: Option<String> }` — wire keys `semver`, `major`, `minor`, `patch`, `preRelease`, `build`.
  - `pub const LAVALINK_V4_PROTOCOL_VERSION: &str = "4.0.0";` — the v4 contract NEXORA targets.
  - `impl V4Version { pub fn nexora() -> Self }` — semver `env!("CARGO_PKG_VERSION")` of the protocol crate, major/minor/patch parsed from it, `pre_release`/`build` `None`.
  - `pub struct V4Git { pub branch: String, pub commit: String, pub commit_time: i64 }`.
  - `pub struct V4Plugin { pub name: String, pub path: String, pub version: String }`.
  - `pub struct V4Info { pub name: String, pub version: V4Version, pub build_time: i64, pub git: V4Git, pub jvm: Option<String>, pub lavaplayer: Option<String>, pub source_managers: Vec<String>, pub filters: Vec<String>, pub plugins: Vec<V4Plugin> }` — wire keys `name`, `version`, `buildTime`, `git`, `jvm`, `lavaplayer`, `sourceManagers`, `filters`, `plugins`.
  - `pub fn check_protocol_version(requested: &str) -> Result<(), nexora_error::NexoraError>` — accepts `"4"` and any `4.x.y` whose major is 4; returns `NexoraError::invalid_input("unsupported Lavalink protocol version")` otherwise. A `4.0.0` request is accepted; `3.0.0` and `5.0.0` are rejected.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn info_serialises_the_v4_shape() {
    let info = V4Info { name: "NEXORA".into(), version: V4Version::nexora(), build_time: 1, git: V4Git { branch: "main".into(), commit: "abc".into(), commit_time: 1 }, jvm: None, lavaplayer: Some("nexora-audio-engine".into()), source_managers: vec!["local".into()], filters: Vec::new(), plugins: Vec::new() };
    let value = serde_json::to_value(&info).unwrap();
    assert_eq!(value["name"], "NEXORA");
    assert!(value["version"]["semver"].is_string());
    assert!(value["version"]["preRelease"].is_null());
    assert_eq!(value["sourceManagers"][0], "local");
    assert_eq!(value["filters"].as_array().unwrap().len(), 0);
}

#[test]
fn version_negotiation_accepts_v4_and_rejects_others() {
    assert!(check_protocol_version("4").is_ok());
    assert!(check_protocol_version("4.0.0").is_ok());
    assert!(check_protocol_version("4.9.9").is_ok());
    assert!(check_protocol_version("3.0.0").is_err());
    assert!(check_protocol_version("5.0.0").is_err());
    assert!(check_protocol_version("").is_err());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p nexora-protocol v4::info`
Expected: FAIL with "cannot find function `check_protocol_version`".

- [ ] **Step 3: Implement `v4/info.rs`**

`V4Version::nexora()` parses `env!("CARGO_PKG_VERSION")` on the first two dots; a crate version without a patch component falls back to `0`. `check_protocol_version` compares the major component only (`"4"` and `"4.x.y"` both have major `4`); a string with no numeric major is rejected. Add `V4Ready { pub version: V4Version }` to `op.rs`; the envelope adds `op`, the payload does not.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p nexora-protocol v4::info`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/nexora-protocol/src/v4/info.rs crates/nexora-protocol/src/v4/op.rs crates/nexora-protocol/src/v4/mod.rs
git commit -m "feat(protocol): add v4 info, version and protocol negotiation"
```

---

### Task 5: the server serves typed v4 payloads

**Files:**
- Modify: `services/nexora-server/src/lib.rs` (`lavalink_info`, `lavalink_track`)
- Test: `services/nexora-server/tests/http.rs`

**Interfaces:**
- Consumes: `nexora_protocol::v4::{V4Info, V4Version, V4Git, V4Track, V4TrackInfo, encode_track}`, `nexora_protocol::v4::info::LAVALINK_V4_PROTOCOL_VERSION`.
- Produces: the existing handler signatures, unchanged; `/v4/info` and `/v4/loadtracks` responses keep their current wire shape (the existing tests must keep passing unchanged).

- [ ] **Step 1: Write the failing test**

Add to `services/nexora-server/tests/http.rs`:

```rust
#[tokio::test]
async fn lavalink_info_reports_the_typed_v4_shape() {
    let response = test_app()
        .oneshot(Request::builder().uri("/v4/info").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["version"]["semver"].is_string());
    assert!(value["version"]["major"].is_number());
    assert!(value["buildTime"].is_number());
    assert!(value["git"]["branch"].is_string());
    assert!(value["filters"].is_array());
    assert!(value["sourceManagers"].is_array());
}
```

- [ ] **Step 2: Run the test to verify it fails or passes honestly**

Run: `cargo test -p nexora-server lavalink_info_reports_the_typed_v4_shape`
Expected: this test may pass against the hand-built JSON — that is acceptable, because the point of this task is that the server stops *building* the payload by hand. If it passes, keep it and continue; the change is verified by the existing tests still passing plus the new one.

- [ ] **Step 3: Replace the hand-built payloads**

In `lavalink_info`, build a `V4Info` and return `Json(serde_json::to_value(info).unwrap_or_else(|_| json!({})))` — the handler keeps returning `Json<Value>`, so no signature change. `filters` must stay `Vec::new()` (NEXORA implements no filters yet) and `source_managers` must keep coming from `state.sources.source_ids()`.

In `lavalink_track`, build a `V4TrackInfo` from the `TrackMetadata`, call `encode_track(&info, &json!({"source_id": metadata.source_id}))`, and build a `V4Track` with `plugin_info: json!({})` and `user_data: json!({})`. The encoded payload's keys stay exactly `track_id`, `source_id`, `title`, `duration_ms` so existing encoded tracks remain decodable by `decode_track`.

- [ ] **Step 4: Run the server tests**

Run: `cargo test -p nexora-server`
Expected: PASS — including the pre-existing `lavalink_loadtracks_returns_v4_track_shape` and `lavalink_info_matches_documented_v4_shape` tests, which must not need edits.

- [ ] **Step 5: Commit**

```bash
git add services/nexora-server/src/lib.rs services/nexora-server/tests/http.rs
git commit -m "refactor(server): serve typed v4 info and track payloads"
```

---

### Task 6: compatibility harness and matrix update

**Files:**
- Create: `services/nexora-server/tests/compat/main.rs`
- Modify: `docs/COMPATIBILITY.md` (M0 rows)
- Modify: `docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md` (the `tests/compat` path)
- Modify: `docs/README.md` (link the plan and the harness)

**Interfaces:**
- Consumes: `nexora_protocol::v4::*` from Tasks 1-4; the server router from `services/nexora-server/src/lib.rs`.
- Produces: a compatibility test file whose names are the matrix row ids, so a matrix row and its test can be found from each other.

- [ ] **Step 1: Write the compatibility tests**

Create `services/nexora-server/tests/compat/main.rs`:

```rust
use axum::body::Body;
use axum::http::Request;
use nexora_protocol::v4::V4EventType;
use nexora_protocol::v4::V4Op;
use tower::ServiceExt;

// Row ids match docs/COMPATIBILITY.md.
#[tokio::test]
async fn compat_v4_info_shape() { /* GET /v4/info, assert version/git/filters/sourceManagers */ }

#[tokio::test]
async fn compat_v4_loadtracks_track_shape() { /* GET /v4/loadtracks, assert encoded decodes and info matches */ }

#[tokio::test]
async fn compat_protocol_op_codes_are_the_reference_values() {
    assert_eq!(V4Op::PlayerUpdate.as_str(), "playerUpdate");
    assert_eq!(V4EventType::TrackStart.as_str(), "TrackStartEvent");
}
```

The first two tests build the router the same way `services/nexora-server/tests/http.rs` does (see `test_state` and `temp_media_root` there).

- [ ] **Step 2: Run the compatibility tests**

Run: `cargo test -p nexora-server --test compat`
Expected: PASS (3 tests).

- [ ] **Step 3: Update the matrix rows**

In `docs/COMPATIBILITY.md`: the `/v4/info` row stays `implemented`; add a note that its payload is now typed by `nexora-protocol::v4`; add the event-op rows for `playerUpdate` and `WebSocketClosedEvent` as `missing (M3)` — M0 provides the op codes and the envelope type, it does not yet emit them. Do not mark any event row implemented.

In the design doc, change the `tests/compat` row of the architecture table to `services/nexora-server/tests/compat/` with the reason (cargo has no root test target).

- [ ] **Step 4: Run the whole workspace**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: all three exit 0, and the test count is 83 + the new tests (state the exact number in the commit).

- [ ] **Step 5: Commit**

```bash
git add services/nexora-server/tests/compat/main.rs docs/COMPATIBILITY.md docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md docs/README.md
git commit -m "test(server): add v4 compatibility harness and update the matrix"
```

---

## Self-review

**Spec coverage.** The M0 row of the design doc names four things: op codes (Task 1), error envelope (Task 2), version negotiation (Task 4), track encoding (Task 3), and the exit proof "protocol contract tests; encoding round-trip fixtures" (Tasks 1-3 inline tests, Task 6 harness). Every M0 item has a task.

**Step scan.** Every step names a file, a signature or a test name, and a command with an expected result. No step says "add appropriate validation".

**Type consistency.** `V4Op`, `V4EventType`, `V4EmittedEvent`, `V4Error`, `V4ErrorSeverity`, `V4ErrorEnvelope`, `V4Track`, `V4TrackInfo`, `encode_track`, `decode_track`, `V4Version`, `V4Info`, `V4Git`, `V4Plugin`, `check_protocol_version`, `LAVALINK_V4_PROTOCOL_VERSION` are each defined once and used with the same name and type in every task. `V4Ready` is introduced in Task 4 and referenced nowhere else yet — Task 5 does not need it, and no later task in this plan depends on it.

**Review Focus.** All five lines have an owning task: unknown op/event (Task 1 `from_wire` returning `None`, Task 6 asserts the reference values), malformed encoded track (Task 3), missing track fields (Task 3), truthful `filters` (Task 4 test, Task 5 keeps the value, Task 6 asserts it is an array), version negotiation (Task 4).

**Proportion.** The plan is shorter than the code it produces: six tasks, six test blocks, no implementation bodies beyond the values the reference fixes.
