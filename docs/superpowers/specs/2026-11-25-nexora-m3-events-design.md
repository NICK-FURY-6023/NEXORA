# NEXORA M3 — Lavalink v4 events — Design

**Milestone:** M3 in `docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md`:
"Events: `playerUpdate`, `TrackStart/End/Exception`, queue events | event ordering
tests against recorded Lavalink behaviour".

**Status:** approved by the standing instruction to complete M3 without pausing
and to push the result to GitHub.

---

## Intent

A Lavalink client drives a music bot over `/v4/websocket`. It sends commands
over REST and learns what happened over the socket: a track started, a track
ended, the player's position moved. Without those frames a client cannot
advance its own queue, cannot show a progress bar, and cannot tell a user why
a track failed. M3 is the milestone that makes the v4 surface usable by a real
client.

**Why now:** M0–M2 made the protocol types, the socket and the session routes
correct. The frames that carry the session's *behaviour* are the last piece
before a client can play a track end to end.

---

## Goals

- Every frame the reference sends on `/v4/websocket` is sent by NEXORA, with
  the same envelope, the same field names, the same values and the same
  ordering.
- A client that reads `TrackEndEvent.reason` can decide whether to advance its
  own queue, exactly as the reference's `finished`/`loadFailed` reasons allow.
- A client that reads `playerUpdate.state.position` sees a progress bar that
  moves, and one that reads `TrackStartEvent.track` knows what is playing.
- The NodeLink events NEXORA chooses to support are typed and emitted with
  NodeLink's payloads.

## Non-goals

- **Discord voice.** The reference's `connected` and `ping` fields come from
  the voice gateway (M5). NEXORA has no gateway, so it reports `connected:
  false` and `ping: -1` — the same values the reference reports when the
  gateway is absent.
- **The reference's plugin hooks.** `SocketContext`'s `EventEmitter` also
  notifies `PluginEventHandler`s; NEXORA has no Java plugin API (M10).
- **Crossfade, gapless and mix events.** NodeLink's `mixing`, `gapless` and
  `crossfading` are separate capabilities, not part of the Lavalink event
  surface M3 closes.
- **Filters.** The filter *types* are already typed; applying them to PCM is
  M4.

---

## Constraints

- Rust stable, edition 2024; `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace` green before the milestone closes.
- No `unwrap`, `expect` or `panic!` in production paths that can fail at
  runtime; errors flow through `nexora-error`.
- Every behavioural claim cites evidence: REA `ev_…`, craft-anything `ev_…`,
  or `spec:` for a line in `docs/research/lavalink-source-architecture.md`.
- Documentation layout follows ADR-021: new plans and specs are linked from
  `docs/README.md`; architecture decisions are new ADRs, never rewritten old
  ones.

---

## Evidence

All Lavalink facts below are read from the shipped `Lavalink.jar` 4.2.2
(sha256 `8cb801e591072c3689fafd71ccf571a95a4ead3cc35dfc045e157d763d89119a`)
or from NodeLink's analysed source tree at `E:\nodelink-src.dtcD`.

| Fact | Value | Evidence |
|---|---|---|
| Event envelope is flat | `op`, `type`, `guildId` and the event's own fields are siblings; there is no `data` wrapper | craft-anything `ev_b36bd60abddf009a8e51ddc93d343f99f9ddf7450da9c2262e5c7f904dad25bb` |
| `TrackStartEvent` payload | `{guildId, track}` | `ev_b36bd60abddf009a8e51ddc93d343f99f9ddf7450da9c2262e5c7f904dad25bb` |
| `TrackEndEvent` payload | `{guildId, track, reason}` | `ev_b36bd60abddf009a8e51ddc93d343f99f9ddf7450da9c2262e5c7f904dad25bb` |
| `TrackExceptionEvent` payload | `{guildId, track, exception{message, severity, cause, causeStackTrace}}` | `ev_b36bd60abddf009a8e51ddc93d343f99f9ddf7450da9c2262e5c7f904dad25bb` |
| `TrackStuckEvent` payload | `{guildId, track, thresholdMs}` | `ev_b36bd60abddf009a8e51ddc93d343f99f9ddf7450da9c2262e5c7f904dad25bb` |
| `playerUpdate` payload | `{op:"playerUpdate", guildId, state{time, position, connected, ping}}` | `ev_286d7a0a8638439ee833681b0e9b0cee146077e06e3b6cd0f01c10fddea932d0` |
| `playerUpdate` cadence | `ServerConfig.playerUpdateInterval`, default **5 seconds**; `scheduleAtFixedRate` with an initial delay of 0 | `ev_286d7a0a8638439ee833681b0e9b0cee146077e06e3b6cd0f01c10fddea932d0` |
| `playerUpdate` is skipped while paused | `sendPlayerUpdate` returns immediately when `sessionPaused` | `ev_286d7a0a8638439ee833681b0e9b0cee146077e06e3b6cd0f01c10fddea932d0` |
| `onTrackStart` does not emit the event | it schedules the `playerUpdate` task; the event comes from a separate `EventEmitter` | `ev_286d7a0a8638439ee833681b0e9b0cee146077e06e3b6cd0f01c10fddea932d0` |
| `onTrackEnd` cancels the update task | `updateFuture.cancel(0)` | `ev_286d7a0a8638439ee833681b0e9b0cee146077e06e3b6cd0f01c10fddea932d0` |
| End-marker reason override | the reason becomes `finished` and `endMarkerHit` resets to false | `ev_c4b9dea83afa532cc23248d41a3f96d4974680b3c0abe6b64b6e4fdfdb60cc6e` |
| Reason mapping | `finished`, `loadFailed`, `stopped`, `replaced`, `cleanup` — one to one | `ev_c4b9dea83afa532cc23248d41a3f96d4974680b3c0abe6b64b6e4fdfdb60cc6e` |
| Frame ordering | every frame goes through one `SocketContext.send` into one Undertow `WebSocketChannel`; while paused the frames queue and replay in insertion order | `ev_d19b17634f58678b44faf005308229642266cb6b9de03badb60287aed895b542` |
| NodeLink extra events | `SeekEvent{position,duration}`, `PauseEvent{paused}`, `VolumeChangedEvent{volume}`, `FiltersChangedEvent{filters}`, `PlayerCreatedEvent{guildId,player}`, `PlayerDestroyedEvent{guildId}` | `ev_b36bd60abddf009a8e51ddc93d343f99f9ddf7450da9c2262e5c7f904dad25bb` |
| NodeLink's envelope | `op:"event"`, `type`, `guildId`, then the payload spread in — the same flat shape | `ev_b36bd60abddf009a8e51ddc93d343f99f9ddf7450da9c2262e5c7f904dad25bb` |

---

## Approaches

Three ways to get the frames out of NEXORA's playback pipeline were
considered.

### A. A single broadcast bus in the v4 session manager

The v4 session manager owns one `broadcast::Sender<V4EventMessage>`; every
producer — the playback task, the player routes — sends into it; every
connected socket subscribes and forwards.

- **For:** one queue, one ordering, no plumbing between the native session
  manager and the v4 surface. The v4 session manager already exists and
  already owns the players, so the players' state lives next to the events
  that describe it.
- **Against:** the native `SessionManager` and the v4 `V4SessionManager` are
  separate stores, so the playback task would have to publish to both.

### B. Extend the native session manager's event channel

The native `SessionManager` already broadcasts `Op` events (playback state
changes) to its own subscribers. M3 would add the v4 frames to that channel
and translate them at the socket.

- **For:** one channel for the whole server; the playback task already
  publishes there.
- **Against:** the native channel's `Op` type is a native-shaped enum, and
  the v4 frames need a guild id, a v4 track and a v4 reason — none of which
  the native events carry. Every translation would need a lookup back into
  the v4 session manager, which is the coupling this option was meant to
  avoid.

### C. A dedicated v4 event bus keyed by session

A new `V4EventBus` owns the channel; the v4 session manager and the playback
task both publish to it.

- **For:** explicit about who owns the events.
- **Against:** a third place to look for "where do v4 frames come from", for
  a surface that already has a session manager holding exactly the state the
  frames describe.

### Chosen: A

One broadcast channel inside the v4 session manager, and the playback task
publishes into it through the `AppState`. The v4 session manager already owns
the players, so the state a frame describes and the frame itself live
together. A single channel gives a single ordering for free — the property
the reference gets from its single WebSocket channel — and it makes the
ordering testable without a socket.

**Why the playback task publishes rather than the socket:** the reference's
ordering guarantee comes from one channel per socket. NEXORA's socket is a
subscriber of a shared channel, so several sockets see the same frames in the
same order. That is what the reference does too — one `SocketContext` per
socket, but the frames originate in the player.

---

## The design

### The channel

`V4SessionManager` keeps one `broadcast::Sender<V4EventMessage>`. It is
already there. `emit` and `subscribe` stay as they are.

**Bound:** `broadcast::channel(256)` keeps 256 frames. A slow socket that
falls further behind than that gets `RecvError::Lagged` and misses frames.
The reference does not drop frames — it writes to the socket and blocks. The
socket loop already handles `Lagged` by continuing; M3 raises the bound and
records the lag rather than silently skipping.

### Where the frames come from

| Frame | Trigger | Emitter |
|---|---|---|
| `TrackStartEvent` | a track starts playing | the playback task, when it begins a track |
| `TrackEndEvent` | a track stops playing | the playback task, on natural end, on stop, and on replacement |
| `TrackExceptionEvent` | a track fails to decode or resolve | the playback task, on a decode or resolve failure |
| `TrackStuckEvent` | playback stops progressing | out of scope for M3 — NEXORA's pipeline is not a live stream, so it cannot stall the way the reference's does |
| `playerUpdate` | every 5 seconds while a track is playing, plus once immediately when a track starts | a per-player task |
| `WebSocketClosedEvent` | a socket closes | the socket loop |
| `SeekEvent` | a seek completes | the player route |
| `PauseEvent` | a player is paused or resumed | the player route |
| `VolumeChangedEvent` | a player's volume changes | the player route |
| `FiltersChangedEvent` | a player's filters change | the player route |
| `PlayerCreatedEvent` | a player is created | the player route |
| `PlayerDestroyedEvent` | a player is removed | the player route |

### Ordering

The reference's order for one track is:

1. `TrackStartEvent`
2. `playerUpdate` (immediately — the scheduled task's initial delay is 0)
3. `playerUpdate` (every 5 seconds)
4. `TrackEndEvent`

The update task is cancelled when the track ends, so no `playerUpdate` follows
the `TrackEndEvent`.

NEXORA reproduces this exactly: the playback task publishes `TrackStartEvent`
and then starts the per-player update task; the update task is cancelled when
the track ends, and `TrackEndEvent` is published after the cancellation.

### The `playerUpdate` task

The reference's task is per player, not per session, and it is tied to the
track's lifetime. NEXORA's is the same:

- It starts when a track starts and stops when the track ends.
- Its interval is 5 seconds, the reference's `playerUpdateInterval` default.
- Its first frame is sent immediately, matching the reference's initial delay
  of 0.
- It is skipped entirely while the player is paused — the reference returns
  before building the frame, so a paused player produces no frames at all.

The frame's `state` is `{time, position, connected, ping}`:
`time` is the current Unix time in milliseconds; `position` is the playing
track's position in milliseconds, or 0 when nothing is playing; `connected` is
false and `ping` is -1 because NEXORA has no voice gateway — the same values
the reference reports with no gateway.

### The `TrackEnd` reason

The reference's reason is the reason the track stopped, except that a track
that reached its end marker reports `finished` regardless of what Lavaplayer
said. NEXORA's reasons:

| Situation | Reason |
|---|---|
| the track played to its end | `finished` |
| the source failed to decode or resolve | `loadFailed` |
| the player was stopped | `stopped` |
| another track replaced it | `replaced` |

`cleanup` is typed but never emitted — it belongs to the player's destruction,
which NEXORA does not signal over the wire.

### The queue

The reference's v4 protocol has **no** queue surface: the queue lives in the
client. The reference's `TrackEndEvent` tells the client whether it should
advance (`finished` and `loadFailed`), and the client then sends the next
track.

NEXORA keeps its own queue in the native session manager, because NEXORA
drives the audio itself. So NEXORA does both: it advances its own queue when a
track ends, and it publishes `TrackEndEvent` with the reason so a v4 client
that also keeps a queue can advance its own. The reference's `finished` and
`loadFailed` are exactly the reasons that mean "advance".

### The NodeLink extras

The six extras are typed as one `V4EventMessage` family with their
NodeLink payloads, and they are emitted from the v4 player routes, which are
where a client changes volume, filters, position or the pause state.

`PlayerCreatedEvent` carries the player object; `PlayerDestroyedEvent`
carries only the guild id, because NodeLink's own payload is exactly that.

---

## The parts

| File | Change |
|---|---|
| `crates/nexora-protocol/src/v4/messages.rs` | add the six NodeLink event variants and the `gapless`/`crossfading` reasons |
| `crates/nexora-protocol/src/v4/player.rs` | expose the player's live position so a frame can carry it |
| `crates/nexora-session/src/v4.rs` | own the per-player update task; publish the frames |
| `services/nexora-server/src/lavalink.rs` | emit the extras from the player routes; stop the update task on track end |
| `services/nexora-server/src/lib.rs` | publish `TrackStartEvent`, `TrackEndEvent` and `TrackExceptionEvent` from the playback task |
| `services/nexora-server/tests/compat/main.rs` | the ordering tests |
| `docs/COMPATIBILITY.md` | the M3 rows move to `implemented` with their proof names |
| `docs/PROJECT_STATUS.md` | the milestone closes |

---

## The exit proof

The M3 exit proof is "event ordering tests against recorded Lavalink
behaviour". NEXORA cannot run the reference server on this host, so the proof
is: the ordering and the payloads are asserted against the recorded reference
behaviour — the bytecode facts above — by tests that read the real frames off
a real socket.

1. `compat_v4_event_order_track_start_update_end` — a client that plays a
   track sees the player-created event, then `TrackStartEvent`, then a
   `playerUpdate` immediately.
2. `compat_v4_track_start_event_carries_the_track` — the `TrackStartEvent`
   carries the track the client asked for.
3. `compat_v4_player_update_state_shape` — the `playerUpdate` frame carries
   `state{time, position, connected, ping}` and a `guildId`.
4. `compat_v4_track_end_event_carries_the_reason` — stopping a player
   produces a `TrackEndEvent` with the reason `stopped` and the track that
   was playing.
5. `compat_v4_track_exception_carries_the_cause` — a track that cannot be
   decoded produces a `TrackExceptionEvent` with the exception's message,
   severity and cause.
6. `compat_v4_paused_player_sends_no_player_update` — a paused player
   produces no `playerUpdate`.
7. `compat_v4_player_events_report_the_change` — the NodeLink extras carry
   their NodeLink payloads.

---

## What is left to the builder

Implement the design above. The order is: types first, then the session
manager's event plumbing, then the server's emission points, then the tests,
then the docs.
