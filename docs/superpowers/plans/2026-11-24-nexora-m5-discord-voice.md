# NEXORA M5 — Discord voice / session lifecycle — Implementation Plan

**Goal:** Serve Lavalink v4's Discord voice surface: gateway voice state updates, UDP voice audio transport, Opus packet encryption (XSecret), and DAVE E2EE for voice payloads. A real Lavalink client should be able to connect via v4 WebSocket, send voice state updates, and receive encrypted Opus audio over UDP.

**Architecture:** The v4 WebSocket (`/v4/websocket`) already exists from M1/M3. M5 adds:
- Voice state tracking per session/guild (voice channel, self-deaf, self-mute)
- Voice state update handling (client sends voice state, server echoes back)
- UDP voice socket bound per session for Opus audio transport
- Voice state update dispatch to the voice socket
- Opus audio packet encryption with XSecret (AES-128-CTR over Opus frames)
- DAVE (Discord Audio Voice Encryption) E2EE for voice payloads (to be verified against reference)

**Architecture mapping (from design doc):**
- `crates/nexora-protocol/src/v4/` — owns v4 wire types; add voice state update + voice server update op codes
- `crates/nexora-session/src/v4.rs` — owns per-session voice state (voice channel, self-deaf, self-mute, voice socket endpoint)
- `crates/nexora-audio-pipeline/src/` — already has Opus encoding; add XSecret encryption on Opus packets
- `services/nexora-server/src/lavalink.rs` — v4 WebSocket handshake already handles User-Id/Session-Id; add voice state update handling and UDP voice socket binding
- `services/nexora-server/src/lib.rs` — route `/v4/websocket` already exists; add voice state update message handling

**Tech Stack:** Rust 2024, axum WebSocketUpgrade (already), tokio::net::UdpSocket (new), serde, `nexora-protocol` v4 types, `nexora-audio-pipeline` Opus encoding (already), `nexora-error` for errors.

**Spec:** `docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md` (M5 row). Reference evidence: `docs/research/lavalink-source-architecture.md` (voice state update, voice server update op codes), shipped `Lavalink.jar` (v4 WebSocket op codes, voice state update format).

## M4 → M5 handoff

M4 (filters) is complete and merged to main. M5 builds on:
- `V4Filters` in `crates/nexora-protocol/src/v4/filters.rs` — already applied in `nexora-audio-pipeline::filters.rs`
- `V4PlayerRecord` in `crates/nexora-session/src/v4.rs` — already has `filters: V4Filters`
- `encode_file_packets(..., &filters)` in `crates/nexora-audio-pipeline/src/lib.rs` — applies filters before Opus encoding
- `/v4/websocket` in `services/nexora-server/src/lavalink.rs` — already emits `V4Ready` and forwards `V4EventMessage`

M5 extends:
- `V4PlayerRecord` gains voice state fields (voice channel id, self-deaf, self-mute)
- `V4EventMessage` gains `VoiceStateUpdate` event
- `run_playback` in `services/nexora-server/src/lib.rs` gains XSecret encryption on Opus packets before sending
- `/v4/websocket` gains voice state update handling (client → server voice state changes)

## M4 → M5 handoff (done)
- M4 committed to main as `e9d9218` (fix equalizer per-channel gain bug) and `6994ce4` (feat(v4): serve Lavalink v4 filter chain)
- `docs/COMPATIBILITY.md` updated: M4 filter rows marked implemented
- `docs/PROJECT_STATUS.md` updated: M4 section, 153 tests

## M5 Tasks

### Task 1: Voice state types in protocol
**Files:**
- Create/modify: `crates/nexora-protocol/src/v4/messages.rs` — add `VoiceStateUpdate` event variant to `V4EventMessage`
- Test: inline `#[cfg(test)]` in `messages.rs`

**Interfaces:**
- `V4EventMessage::VoiceStateUpdate { guild_id, user_id, channel_id, self_deaf, self_mute }` — wire keys `guild_id`, `user_id`, `channel_id`, `self_deaf`, `self_mute`
- Wire values: `VoiceStateUpdate` event type

### Task 2: Voice state in session record
**Files:**
- Modify: `crates/nexora-session/src/v4.rs` — add voice state fields to `V4PlayerRecord`
- Test: inline tests in `v4.rs`

**Interfaces:**
- `V4PlayerRecord` gains: `voice_channel_id: Option<String>`, `self_deaf: bool`, `self_mute: bool`
- `V4PlayerRecord::new` initializes: `voice_channel_id: None`, `self_deaf: false`, `self_mute: false`

### Task 3: Voice state update handling in v4 WebSocket
**Files:**
- Modify: `services/nexora-server/src/lavalink.rs` — voice state update handling in websocket handler
- Test: `services/nexora-server/tests/compat/main.rs` — voice state update round-trip

**Interfaces:**
- Client sends `{"op": 4, "d": {"guild_id": "...", "user_id": "...", "channel_id": "...", "self_deaf": false, "self_mute": false}}`
- Server echoes back `VoiceStateUpdate` event
- Op code `4` = voice state update (Lavalink v4 convention)

### Task 4: UDP voice socket per session
**Files:**
- Modify: `services/nexora-server/src/lib.rs` — `start_playback` binds UDP socket per session
- Modify: `services/nexora-server/src/lavalink.rs` — websocket handler binds UDP socket on session create

**Interfaces:**
- UDP socket bound to `0.0.0.0:0` per session
- Opus packets sent over UDP to Discord voice gateway endpoint
- Session tracks UDP socket address for voice gateway

### Task 5: XSecret encryption on Opus packets
**Files:**
- Modify: `crates/nexora-audio-pipeline/src/lib.rs` — `encode_file_packets` applies XSecret encryption
- Modify: `crates/nexora-audio-pipeline/src/filters.rs` — XSecret encryption module

**Interfaces:**
- XSecret = AES-128-CTR encryption of Opus packets
- Key derived from voice session key (shared secret from voice gateway handshake)
- IV = sequence number (4-byte big-endian)

### Task 6: DAVE E2EE (to be verified)
**Files:**
- `crates/nexora-audio-pipeline/src/filters.rs` — DAVE encryption placeholder

**Interfaces:**
- DAVE = Discord Audio Voice Encryption — end-to-end encryption for voice payloads
- To be verified against reference during M5; may be documented as not-targeted if not verifiable from available evidence

## Exit proof
- `cargo test --workspace` green
- `cargo clippy --workspace --all-targets -- -D warnings` clean
- `cargo fmt --all -- --check` clean
- Compat tests: voice state update round-trip, UDP socket binding, XSecret encryption on Opus packets
- docs/COMPATIBILITY.md: M5 rows updated
- docs/PROJECT_STATUS.md: M5 section added
