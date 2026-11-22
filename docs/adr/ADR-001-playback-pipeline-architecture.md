# ADR-001: Playback pipeline architecture

## Context

NEXORA needed a real-time local-file playback slice: a client creates a session, plays a local track, and receives Opus audio over a dedicated WebSocket while control commands (pause/resume/seek/volume/stop) flow through the REST API and event WebSocket.

The pipeline had to satisfy the Lavalink frame-accounting contract (one packet per 20 ms, monotonic sequence and timestamp) while keeping audio out of the control path, bounding memory, and keeping volume correct without mutating encoded packets.

## Decision

Implement the playback pipeline as a per-session pacing task with the following structure:

- **Resolution before state change.** `POST /play` resolves the track to a `ResolvedSource::LocalFile` first. If resolution fails or the source is not a local file, the session is not marked playing and a `TrackFailed` event is emitted.
- **One pacing task per session.** A single spawned task owns playback for a session, driven by a `tokio::time::interval` 20 ms ticker and a `watch::Receiver<PlaybackSignal>` for control commands.
- **Prepared Opus packets.** `encode_file_packets(path, rate, channels, volume)` decodes (Symphonia), resamples (Rubato FFT), remaps channels, applies PCM gain, and encodes to Opus 20 ms packets before pacing begins. Volume changes re-encode from the current position with the new gain.
- **Bounded broadcast delivery.** A `broadcast::Sender<OpusFrame>` per session fans packets to the audio WebSocket. Lagging consumers skip missed packets (`RecvError::Lagged(n)`) rather than blocking the producer.
- **PCM gain before encoding.** Volume (0..150) is applied as PCM gain clamped to -1.0..1.0 before encoding, so encoded packets are never mutated and gain is always correct at the current position.
- **Session state owns position and status.** `SessionManager` tracks `status`, `position_ms`, `volume`, `current_track`, and `queue`; the pacing task advances `position_ms` via `SessionManager::update_position`.
- **End-of-track and failure handling.** End of packets emits `TrackEnded` and clears the track; decode failure emits `TrackFailed`. `finish()` is terminal; `pop_next_queued()` exposes FIFO order but queue auto-advance is deferred to a later slice — the next slice will wire `finish()` → `pop_next_queued()` → `play()`.

## Consequences

Positive:

- Audio never shares the control WebSocket; the event channel stays text-only and the audio channel is binary Opus.
- Bounded channels prevent slow clients from growing memory without limit.
- Volume changes are sample-accurate because gain is applied to PCM before re-encode.
- Resolution-before-state-change means a failed play never leaves a session in `playing` state.

Negative / tradeoffs:

- Whole-file packet preparation is acceptable for the first slice but bounded by a documented local-file size policy; streaming decode from a reader is a later optimization.
- Queue auto-advance is deferred; `finish()` is terminal and does not spawn the next playback task.
- One pacing task per session does not yet scale to many concurrent sessions without a worker pool.

## Alternatives

- **Streaming decode + live encode:** Rejected for slice 1 because it adds reader/lifetime complexity and the first slice targets bounded local files.
- **Audio through the REST API as base64:** Rejected because it violates the control/data plane separation and adds per-message JSON overhead.
- **Unbounded queue:** Rejected (see ADR-020 bounded audio queues) because it hides overload and can exhaust memory.

## Status

Accepted; implemented in the real-time playback slice (Phase 4a remainder). See `docs/SYSTEM_DESIGN.md` slice 1 and `services/nexora-server/src/lib.rs` (`run_playback`, `prepare_packets`, `encode_file_packets`).
