# ADR-023: Queue auto-advance inside the pacing task

## Status

Accepted, 2026-11-23. Implements the deferral recorded in ADR-022.

## Context

ADR-022 shipped one pacing task per session and deliberately deferred queue
auto-advance to slice 2: when a track's packets ran out, `finish()` was
terminal and the session stopped, even when the queue still held tracks.

Two properties made the deferred design ambiguous:

- `SessionManager::finish()` clears the session's playback handle and sets the
  session to `stopped`, so it cannot be used on the advance path without
  leaving a live pacing task detached from its session.
- `docs/SYSTEM_DESIGN.md` described the advance as "calls `play()` for it,
  which starts a new pacing task". The HTTP `play` path registers a **new**
  `broadcast::Sender<OpusFrame>` per call, so a second task would replace the
  sender and every audio WebSocket already connected to the session would stop
  receiving packets.

## Decision

The pacing task advances the queue itself, in place:

- On end-of-packets the task calls `pop_next_queued()`. With an empty queue it
  calls `finish()` and stops, exactly as before.
- With a queued track it calls the new `SessionManager::end_track()`, which
  clears the finished track and emits `TrackEnded` while leaving the session's
  playback handle attached, then resolves the next track through the same
  `SourceRegistry` and `internal_auth()` the HTTP `play` path uses.
- When the next track resolves and its packets are prepared, the task calls
  `SessionManager::play()` for it, which emits `TrackStarted` and resets
  `position_ms` to 0. The ticker is reset so the 20 ms cadence restarts
  cleanly after the preparation gap.
- A failed resolve or decode emits `TrackFailed` and stops the session.

The same task, the same `broadcast::Sender<OpusFrame>` and the same
`watch::Receiver<PlaybackSignal>` therefore serve every track in the queue.

## Consequences

Positive:

- Connected audio WebSocket clients keep receiving packets across a track
  change; no re-subscribe is needed and no packet gap is introduced by a
  channel swap.
- Pause, resume, seek and volume keep working across the boundary because the
  control channel is never replaced.
- `TrackEnded` and `TrackStarted` are emitted in order, so clients can render
  the transition.

Negative / tradeoffs:

- One pacing task still serves one session, so many concurrent sessions still
  need a worker pool; that stays on the roadmap.
- Packet preparation for the next track blocks the pacing task for the duration
  of the decode, so the gap between two tracks is the preparation time rather
  than exactly 20 ms.
- Only local files can be advanced into, because the resolve path accepts
  `ResolvedSource::LocalFile` only, the same restriction the HTTP `play` path
  has.

## Alternatives

- **A new pacing task per track:** rejected because it replaces the per-session
  audio broadcast sender and silently drops already-connected subscribers.
- **Auto-advancing inside `SessionManager`:** rejected because ADR-022 gives
  advancement to the playback task; session state should not start a track the
  task never prepared.
- **Reusing `finish()` for the advance path:** rejected because it clears the
  playback handle and stops the session, which would strand a live task.

## Status

Accepted; implemented in `crates/nexora-session/src/lib.rs` (`end_track`) and
`services/nexora-server/src/lib.rs` (`run_playback`), with the integration test
`playback_advances_to_the_next_queued_track` in
`services/nexora-server/tests/http.rs`. Live smoke: the event WebSocket reported
`track_started` → `track_ended` → `track_started` for the queued track, and the
audio WebSocket received 256 Opus packets with no drop across the change.
