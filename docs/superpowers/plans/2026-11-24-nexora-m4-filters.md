# NEXORA M4 — Lavalink v4 filters — Implementation Plan

**Goal:** Apply Lavalink v4's ten built-in filters (volume, equalizer, karaoke, timescale, tremolo, vibrato, distortion, rotation, channelMix, lowPass) plus NodeLink extras (highpass, compressor, echo, phaser) to PCM audio before Opus encoding. Every filter parameter name and default matches the shipped reference.

**Architecture:** A new filter DSP module in `crates/nexora-protocol/src/nexora/filters.rs` (types already exist) plus `crates/nexora-audio-pipeline/src/filters.rs` (DSP implementation). The playback task applies the per-player `NexoraFilters` to interleaved f32 PCM before Opus encoding. The nexora `PATCH /nexora/sessions/{id}/players/{guildId}` route already stores `NexoraFilters` on the `NexoraPlayerRecord`; M4 wires that stored filter chain into the audio pipeline.

**Tech Stack:** Rust 2024, rubato (already a workspace dependency) for timescale, `nexora-error` for errors, `nexora-protocol::nexora::filters` for types.

**Spec:** `docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md` (M4 row). Reference evidence: `filterConfigs.kt` and `Version` class in `protocol-jvm-4.2.2.jar`; `src/typings/playback/filters.types.ts` in NodeLink source.

## M4 File structure

```
crates/nexora-audio-pipeline/src/
  lib.rs              # MODIFIED: apply_filters in encode path; add filters module
  filters.rs          # NEW: DSP filter implementations (14 filters)
crates/nexora-protocol/src/nexora/filters.rs  # EXISTING: types already present, no change needed
services/nexora-server/src/nexora.rs  # MODIFIED: pass player filters into encode path
services/nexora-server/tests/http.rs    # MODIFIED: filter end-to-end test
docs/COMPATIBILITY.md                       # MODIFIED: M4 filter rows -> implemented
```

## M4 Interfaces

### `crates/nexora-audio-pipeline/src/filters.rs`

```rust
/// Apply a Lavalink v4 filter chain to interleaved f32 PCM.
/// Filters are applied in the reference's declaration order: volume, equalizer,
/// karaoke, timescale, tremolo, vibrato, distortion, rotation, channelMix,
/// lowPass, highpass, compressor, echo, phaser.
pub fn apply_filters(
    samples: &mut [f32],
    channels: usize,
    sample_rate: u32,
    filters: &NexoraFilters,
) -> NexoraResult<()>
```

Each filter is an `f32` in-place DSP operation:
- `volume`: multiply every sample by gain
- `equalizer`: per-band biquad peaking filters summed
- `karaoke`: vocal reduction via channel subtraction + bandpass
- `timescale`: rubato/rubato-style time stretch (sample rate conversion with pitch preservation)
- `tremolo`: amplitude modulation by LFO
- `vibrato`: pitch modulation by LFO (delay-line)
- `distortion`: waveshaping (tanh/atan/sin/cos/tan mix)
- `rotation`: stereo channel rotation matrix
- `channelMix`: 2x2 channel mixing matrix
- `lowPass`: one-pole low-pass
- `highPass`: one-pole high-pass
- `compressor`: threshold/ratio/attack/release/makeup gain dynamics
- `echo`: delay feedback echo
- `phaser`: allpass phased notch filter

### `services/nexora-server/src/nexora.rs`

The `PATCH /nexora/sessions/{id}/players/{guildId}` route already stores `NexoraFilters` on `NexoraPlayerRecord.filters`. M4 threads those filters through `start_playback` → `run_playback` → `encode_file_packets` → `apply_filters`.

### `crates/nexora-audio-pipeline/src/lib.rs`

`encode_file_packets(input, format, volume)` gains a `filters: &NexoraFilters` parameter, applied between volume gain and Opus encoding.

## M4 Tests

1. `filter_applies_gain` — volume filter scales samples linearly
2. `filter_low_pass_attenuates_high_freq` — sine sweep through lowPass shows attenuation
3. `filter_equalizer_band_gain` — single band boost measured at band frequency
4. `filter_preserves_sample_count` — filters don't change sample count
5. `filter_chain_empty_is_identity` — default/empty filters leave samples unchanged
6. `encode_file_packets_applies_filters` — encoded Opus decodes back with filter applied

## Self-review

- **Spec coverage:** M4 row names ten Lavalink built-ins + NodeLink extras. Every filter has a struct + DSP impl + wire-name serialization test.
- **Step scan:** each step names a file, signature, test, and command.
- **Type consistency:** `NexoraFilters` from `nexora-protocol::nexora` is used by `apply_filters`; `encode_file_packets` takes `&NexoraFilters`.
- **Review focus:** unknown op/event (M1), malformed encoded track (M3), missing track fields (M3), truthful filters (M4 `enabled_names` + `is_none` defaults), version negotiation (M3).
