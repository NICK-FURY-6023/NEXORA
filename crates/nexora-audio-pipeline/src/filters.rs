//! DSP filter chain implementing the Nexora nexora filter set.
//!
//! Every parameter name and default below matches the shipped reference:
//! `filterConfigs.kt` for Nexora, `src/typings/playback/filters.types.ts`
//! for the Nexora extras.
//!
//! Filters are applied in-place to interleaved f32 PCM, in the reference's
//! declaration order: volume, equalizer, karaoke, timescale, tremolo, vibrato,
//! distortion, rotation, channelMix, lowPass, highpass, compressor, echo,
//! phaser. Filters that are `None` are skipped.

use std::f32::consts::PI;

use crate::NexoraResult;
use nexora_protocol::nexora::{
    NexoraChannelMixFilter, NexoraCompressorFilter, NexoraDistortionFilter, NexoraEchoFilter, NexoraEqualizerBand,
    NexoraFilters, NexoraHighPassFilter, NexoraKaraokeFilter, NexoraLowPassFilter, NexoraPhaserFilter,
    NexoraRotationFilter, NexoraTimescaleFilter, NexoraTremoloFilter, NexoraVibratoFilter,
};

// ---------------------------------------------------------------------------
// Filter chain
// ---------------------------------------------------------------------------

/// Apply the full Nexora nexora filter chain to interleaved f32 PCM, in place.
///
/// Filters are applied in the reference's declaration order: volume, equalizer,
/// karaoke, timescale, tremolo, vibrato, distortion, rotation, channelMix,
/// lowPass, highpass, compressor, echo, phaser. Filters that are `None` are
/// skipped.
pub fn apply_filters(
    samples: &mut [f32],
    channels: usize,
    sample_rate: u32,
    filters: &NexoraFilters,
) -> NexoraResult<()> {
    if let Some(volume) = filters.volume {
        apply_volume(samples, volume);
    }
    if let Some(equalizer) = &filters.equalizer {
        apply_equalizer(samples, channels, sample_rate, equalizer);
    }
    if let Some(karaoke) = &filters.karaoke {
        apply_karaoke(samples, channels, sample_rate, karaoke);
    }
    if let Some(timescale) = &filters.timescale {
        apply_timescale(samples, channels, sample_rate, timescale)?;
    }
    if let Some(tremolo) = &filters.tremolo {
        apply_tremolo(samples, sample_rate, tremolo);
    }
    if let Some(vibrato) = &filters.vibrato {
        apply_vibrato(samples, sample_rate, vibrato);
    }
    if let Some(distortion) = &filters.distortion {
        apply_distortion(samples, distortion);
    }
    if let Some(rotation) = &filters.rotation {
        apply_rotation(samples, channels, rotation);
    }
    if let Some(channel_mix) = &filters.channel_mix {
        apply_channel_mix(samples, channels, channel_mix);
    }
    if let Some(low_pass) = &filters.low_pass {
        apply_low_pass(samples, sample_rate, low_pass);
    }
    if let Some(high_pass) = &filters.highpass {
        apply_high_pass(samples, sample_rate, high_pass);
    }
    if let Some(compressor) = &filters.compressor {
        apply_compressor(samples, sample_rate, compressor);
    }
    if let Some(echo) = &filters.echo {
        apply_echo(samples, sample_rate, echo);
    }
    if let Some(phaser) = &filters.phaser {
        apply_phaser(samples, sample_rate, phaser);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// volume
// ---------------------------------------------------------------------------

fn apply_volume(samples: &mut [f32], gain: f32) {
    for sample in samples.iter_mut() {
        *sample = (*sample * gain).clamp(-1.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// equalizer (peaking biquad per band, summed)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn peaking(sample_rate: u32, gain_db: f32, center_freq: f32, q: f32) -> Self {
        // RBJ peaking biquad, normalized by a0.
        let w0 = 2.0 * PI * center_freq / sample_rate as f32;
        let alpha = w0.sin() / (2.0 * q);
        let a = 10.0_f32.powf(gain_db / 40.0);
        let cos_w0 = w0.cos();
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let out = self.b0 * sample + self.z1;
        self.z1 = self.b1 * sample + self.z2 - self.a1 * out;
        self.z2 = self.b2 * sample - self.a2 * out;
        out
    }
}

/// Build a biquad from equalizer band parameters (gain in dB).
fn eq_band(sample_rate: u32, band: &NexoraEqualizerBand) -> Biquad {
    let gain_db = band.gain;
    let center_freq = band.band as f32;
    // NexoraEqualizerBand carries no Q on the wire, so Q is fixed; upgrade when the wire adds Q.
    // shortcut: fixed Q = 1.0, revisit if the protocol gains a per-band Q field.
    let q = 1.0;
    Biquad::peaking(sample_rate, gain_db, center_freq, q)
}

fn apply_equalizer(
    samples: &mut [f32],
    channels: usize,
    sample_rate: u32,
    bands: &[NexoraEqualizerBand],
) {
    let mut filters: Vec<Biquad> = bands.iter().map(|b| eq_band(sample_rate, b)).collect();
    for frame in 0..samples.len() / channels {
        for channel in 0..channels {
            let idx = frame * channels + channel;
            let mut band_sum = 0.0;
            for filt in filters.iter_mut() {
                band_sum += filt.process(samples[idx]);
            }
            samples[idx] = (samples[idx] + band_sum).clamp(-1.0, 1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// karaoke
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct OnePole {
    coeff: f32,
    state: f32,
}

impl OnePole {
    fn new(sample_rate: u32, freq: f32) -> Self {
        let c = 2.0 * PI * freq / sample_rate as f32;
        let coeff = 1.0 - c / (1.0 + c);
        Self { coeff, state: 0.0 }
    }

    fn process(&mut self, input: f32) -> f32 {
        self.state = self.state + self.coeff * (input - self.state);
        self.state
    }
}

fn apply_karaoke(
    samples: &mut [f32],
    channels: usize,
    sample_rate: u32,
    karaoke: &NexoraKaraokeFilter,
) {
    let mut bandpass = OnePole::new(sample_rate, karaoke.filter_band);
    for frame in 0..samples.len() / channels {
        let base = frame * channels;
        let mono = if channels == 1 {
            samples[base]
        } else {
            (samples[base] + samples[base + 1]) * 0.5
        };
        let filtered = bandpass.process(mono);
        let karaoked = mono - filtered * karaoke.filter_width;
        for channel in 0..channels {
            samples[frame * channels + channel] =
                (mono + karaoked * karaoke.level).clamp(-1.0, 1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// timescale (rubato-style via linear resampling)
// ---------------------------------------------------------------------------

fn apply_timescale(
    samples: &mut [f32],
    channels: usize,
    sample_rate: u32,
    timescale: &NexoraTimescaleFilter,
) -> NexoraResult<()> {
    let rate = timescale.rate;
    if rate == 1.0 {
        return Ok(());
    }
    // Rubato changes sample rate: to slow down (rate < 1), output sample rate
    // is lower relative to input. We emulate by resampling input -> output at
    // input_rate/rate.
    let out_rate = (sample_rate as f32 / rate) as u32;
    let resampled = crate::resample_interleaved(samples, channels, sample_rate, out_rate)?;
    samples.clone_from_slice(&resampled);
    Ok(())
}

// ---------------------------------------------------------------------------
// tremolo
// ---------------------------------------------------------------------------

fn apply_tremolo(samples: &mut [f32], sample_rate: u32, tremolo: &NexoraTremoloFilter) {
    let mut phase = 0.0f32;
    let step = 2.0 * PI * tremolo.frequency / sample_rate as f32;
    let depth = tremolo.depth.clamp(0.0, 1.0);
    for sample in samples.iter_mut() {
        let lfo = 1.0 - depth + depth * phase.sin();
        *sample = (*sample * lfo).clamp(-1.0, 1.0);
        phase += step;
        if phase > 2.0 * PI {
            phase -= 2.0 * PI;
        }
    }
}

// ---------------------------------------------------------------------------
// vibrato
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DelayLine {
    buffer: Vec<f32>,
    write_pos: usize,
}

impl DelayLine {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len],
            write_pos: 0,
        }
    }

    fn write(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
    }

    fn read(&self, delay_samples: f32) -> f32 {
        let n = self.buffer.len();
        let idx = ((self.write_pos as isize - delay_samples.round() as isize)
            .rem_euclid(n as isize)) as usize;
        self.buffer[idx]
    }
}

fn apply_vibrato(samples: &mut [f32], sample_rate: u32, vibrato: &NexoraVibratoFilter) {
    let max_delay = (sample_rate as f32 * 0.02) as usize; // max 20ms delay
    let mut delay_line = DelayLine::new(max_delay);
    let mut phase = 0.0f32;
    let step = 2.0 * PI * vibrato.frequency / sample_rate as f32;
    let depth = vibrato.depth.clamp(0.0, 1.0) * max_delay as f32 * 0.5;
    for sample in samples.iter_mut() {
        delay_line.write(*sample);
        let d = depth * (1.0 + phase.sin()) * 0.5;
        *sample = delay_line.read(d);
        phase += step;
        if phase > 2.0 * PI {
            phase -= 2.0 * PI;
        }
    }
}

// ---------------------------------------------------------------------------
// distortion
// ---------------------------------------------------------------------------

fn apply_distortion(samples: &mut [f32], distortion: &NexoraDistortionFilter) {
    let drive = distortion.sin_scale.clamp(0.0, 10.0);
    for sample in samples.iter_mut() {
        let x = *sample * drive;
        let y = (distortion.sin_offset.sin() * x.sin()
            + distortion.sin_scale * x.sin()
            + distortion.cos_offset.cos() * x.cos()
            + distortion.cos_scale * x.cos()
            + distortion.tan_offset * x.tan()
            + distortion.tan_scale * x.tan()
            + distortion.offset
            + distortion.scale * x)
            .clamp(-1.0, 1.0);
        *sample = y;
    }
}

// ---------------------------------------------------------------------------
// rotation / channelMix
// ---------------------------------------------------------------------------

fn apply_rotation(samples: &mut [f32], channels: usize, rotation: &NexoraRotationFilter) {
    if channels < 2 {
        return;
    }
    let hz = rotation.rotation_hz;
    // rotation acts on stereo pairs
    for frame in 0..samples.len() / channels {
        let base = frame * channels;
        let l = samples[base];
        let r = samples[base + 1];
        let angle = hz * 0.01; // arbitrary rotation speed
        samples[base] = l * angle.cos() - r * angle.sin();
        samples[base + 1] = l * angle.sin() + r * angle.cos();
    }
}

fn apply_channel_mix(samples: &mut [f32], channels: usize, mix: &NexoraChannelMixFilter) {
    if channels < 2 {
        return;
    }
    for frame in 0..samples.len() / channels {
        let base = frame * channels;
        let l = samples[base];
        let r = samples[base + 1];
        samples[base] = (l * mix.left_to_left + r * mix.left_to_right).clamp(-1.0, 1.0);
        samples[base + 1] = (l * mix.right_to_left + r * mix.right_to_right).clamp(-1.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// lowPass / highPass (one-pole)
// ---------------------------------------------------------------------------

fn apply_low_pass(samples: &mut [f32], sample_rate: u32, low_pass: &NexoraLowPassFilter) {
    let mut state = 0.0f32;
    let smoothing = low_pass.smoothing.clamp(0.0, 1.0);
    let c = 2.0 * PI * low_pass_filter_cutoff(sample_rate, smoothing);
    let a = c / (1.0 + c);
    for sample in samples.iter_mut() {
        let input = *sample;
        state = state + a * (input - state);
        *sample = state.clamp(-1.0, 1.0);
    }
}

fn low_pass_filter_cutoff(sample_rate: u32, smoothing: f32) -> f32 {
    // Map smoothing [0,1] to cutoff frequency [20Hz, sample_rate/2]
    let min_freq = 20.0f32;
    let max_freq = sample_rate as f32 / 2.0;
    min_freq + (max_freq - min_freq) * (1.0 - smoothing)
}

fn apply_high_pass(samples: &mut [f32], sample_rate: u32, high_pass: &NexoraHighPassFilter) {
    let mut state = 0.0f32;
    let mut prev_input = 0.0f32;
    let smoothing = high_pass.smoothing.clamp(0.0, 1.0);
    let c = 2.0 * PI * low_pass_filter_cutoff(sample_rate, smoothing);
    let a = c / (1.0 + c);
    for sample in samples.iter_mut() {
        let input = *sample;
        state = a * (state + input - prev_input);
        prev_input = input;
        *sample = state.clamp(-1.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// compressor
// ---------------------------------------------------------------------------

fn apply_compressor(samples: &mut [f32], _sample_rate: u32, compressor: &NexoraCompressorFilter) {
    let threshold = compressor.threshold.clamp(-1.0, 1.0);
    let ratio = compressor.ratio.max(1.0);
    let attack = compressor.attack.clamp(0.0, 1.0);
    let release = compressor.release.clamp(0.0, 1.0);
    let makeup = compressor.makeup_gain.clamp(0.0, 2.0);

    let attack_coeff = (1.0f32 - attack).exp(); // per-sample
    let release_coeff = (1.0f32 - release).exp();

    let mut envelope = 0.0f32;
    for sample in samples.iter_mut() {
        let level = sample.abs();
        envelope = if level > envelope {
            envelope + attack_coeff * (level - envelope)
        } else {
            envelope + release_coeff * (level - envelope)
        };
        let gain = if envelope > threshold {
            (threshold + (envelope - threshold) / ratio) / envelope
        } else {
            1.0
        };
        let gain = gain * makeup;
        *sample = (*sample * gain).clamp(-1.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// echo
// ---------------------------------------------------------------------------

fn apply_echo(samples: &mut [f32], sample_rate: u32, echo: &NexoraEchoFilter) {
    let delay_samples = (sample_rate as f32 * echo.delay / 1000.0) as usize;
    if delay_samples == 0 {
        return;
    }
    let delay = echo.delay.clamp(0.0, 2.0);
    let feedback = echo.feedback.clamp(0.0, 0.95);
    let mix = echo.mix.clamp(0.0, 1.0);

    let delay_samples = (sample_rate as f32 * delay / 1000.0) as usize;
    let mut buffer = vec![0.0f32; delay_samples.max(1)];
    let mut write_pos = 0;

    for sample in samples.iter_mut() {
        let delayed = buffer[write_pos];
        let out = *sample + delayed * feedback * mix;
        buffer[write_pos] = *sample;
        write_pos = (write_pos + 1) % buffer.len();
        *sample = (*sample * (1.0 - mix) + out * mix).clamp(-1.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// phaser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct AllPass {
    coeff: f32,
    state: f32,
}

impl AllPass {
    fn new(sample_rate: u32, freq: f32) -> Self {
        let c = (1.0 - 2.0 * PI * freq / sample_rate as f32).tan()
            / (1.0 + 2.0 * PI * freq / sample_rate as f32).tan();
        Self {
            coeff: c,
            state: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let out = -self.coeff * input + self.state;
        self.state = input + self.coeff * out;
        out
    }
}

fn apply_phaser(samples: &mut [f32], sample_rate: u32, phaser: &NexoraPhaserFilter) {
    let mut allpasses = [
        AllPass::new(sample_rate, phaser.min_frequency),
        AllPass::new(sample_rate, phaser.max_frequency),
    ];
    let mut phase = 0.0f32;
    let step = 2.0 * PI * phaser.rate / sample_rate as f32;
    let depth = phaser.depth.clamp(0.0, 1.0);
    for sample in samples.iter_mut() {
        let mut out = *sample;
        for filt in allpasses.iter_mut() {
            out = filt.process(out);
        }
        let mod_depth = depth * (1.0 + phase.sin()) * 0.5;
        *sample = (*sample * (1.0 - mod_depth) + out * mod_depth).clamp(-1.0, 1.0);
        phase += step;
        if phase > 2.0 * PI {
            phase -= 2.0 * PI;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, sample_rate: u32, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| (2.0 * PI * freq * i as f32 / sample_rate as f32).sin() * 0.5)
            .collect()
    }

    fn peak(samples: &[f32]) -> f32 {
        let mut m = 0.0f32;
        for s in samples {
            m = m.max(s.abs());
        }
        m
    }

    #[test]
    fn filter_chain_empty_is_identity() {
        let mut samples = sine(440.0, 48_000, 960);
        let before = samples.clone();
        apply_filters(&mut samples, 1, 48_000, &NexoraFilters::default()).unwrap();
        assert_eq!(samples, before);
    }

    #[test]
    fn filter_applies_gain() {
        let mut samples = vec![0.5, -0.5, 1.0, -1.0];
        apply_filters(
            &mut samples,
            1,
            48_000,
            &NexoraFilters {
                volume: Some(0.5),
                ..Default::default()
            },
        )
        .unwrap();
        assert!((samples[0] - 0.25).abs() < 1e-6);
        assert!((samples[2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn filter_preserves_sample_count() {
        let mut samples = sine(440.0, 48_000, 1000);
        let n = samples.len();
        apply_filters(&mut samples, 1, 48_000, &NexoraFilters::default()).unwrap();
        assert_eq!(samples.len(), n);
    }

    #[test]
    fn filter_low_pass_attenuates_high_freq() {
        let mut samples = sine(12_000.0, 48_000, 960);
        let before = peak(&samples);
        apply_filters(
            &mut samples,
            1,
            48_000,
            &NexoraFilters {
                low_pass: Some(NexoraLowPassFilter { smoothing: 1.0 }),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(peak(&samples) < before);
    }

    #[test]
    fn filter_equalizer_band_gain() {
        let mut samples = sine(1_000.0, 48_000, 4800);
        let before = peak(&samples);
        apply_filters(
            &mut samples,
            1,
            48_000,
            &NexoraFilters {
                equalizer: Some(vec![NexoraEqualizerBand {
                    band: 1000,
                    gain: 12.0,
                }]),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(peak(&samples) > before);
    }
}

// ---------------------------------------------------------------------------
// XSecret — Opus packet encryption for the nexora voice transport
// ---------------------------------------------------------------------------

// XSecret encrypts each Opus packet with AES-128-CTR before it is sent to
// the Discord voice gateway. The key is derived from the voice session
// token the client supplies on `PATCH /nexora/sessions/{id}/players/{guildId}`
// (`NexoraVoiceState.token`); the counter block is the packet's sequence
// number as a 4-byte big-endian integer, zero-padded to 16 bytes.
//
// NEXORA has no voice gateway handshake yet, so the token is the only key
// material available. Shortcut: the key is the first 16 bytes of the token,
// zero-padded; upgrade to a real KDF once the voice gateway carries a
// shared secret (shortcut: token-as-key material, revisit with the handshake).

use aes::Aes128;
use cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;

use crate::NexoraError;

/// Derives the 16-byte XSecret key from a voice session token.
pub fn xsecret_key(token: &str) -> [u8; 16] {
    let mut key = [0u8; 16];
    let bytes = token.as_bytes();
    let len = bytes.len().min(16);
    key[..len].copy_from_slice(&bytes[..len]);
    key
}

/// Encrypts (or decrypts — CTR is symmetric) `input` with `key`, keyed by
/// `sequence` as the 4-byte big-endian IV.
pub fn xsecret_crypt(key: &[u8; 16], sequence: u32, input: &[u8]) -> NexoraResult<Vec<u8>> {
    let mut counter = [0u8; 16];
    counter[..4].copy_from_slice(&sequence.to_be_bytes());
    let mut cipher = Ctr128BE::<Aes128>::new_from_slices(key, &counter)
        .map_err(|_| NexoraError::service("xsecret cipher construction failed"))?;
    let mut output = input.to_vec();
    cipher.apply_keystream(&mut output);
    Ok(output)
}

#[cfg(test)]
mod xsecret_tests {
    use super::*;

    #[test]
    fn xsecret_decrypts_what_it_encrypts_with_the_same_key_and_sequence() {
        let key = xsecret_key("voice-session-token-alpha");
        let packet: &[u8] = b"opus-payload-bytes";
        let encrypted = xsecret_crypt(&key, 7, packet).expect("encryption works");
        let decrypted = xsecret_crypt(&key, 7, &encrypted).expect("decryption works");
        assert_eq!(&decrypted, packet);
        assert_ne!(&encrypted, packet);
    }

    #[test]
    fn xsecret_output_changes_with_the_sequence_number() {
        let key = xsecret_key("token");
        let packet = b"same-payload";
        let first = xsecret_crypt(&key, 0, packet).expect("encryption works");
        let second = xsecret_crypt(&key, 1, packet).expect("encryption works");
        assert_ne!(first, second);
    }

    #[test]
    fn xsecret_key_is_exactly_16_bytes() {
        assert_eq!(xsecret_key("short").len(), 16);
        assert_eq!(xsecret_key("a-longer-token-than-sixteen-bytes").len(), 16);
    }

    #[test]
    fn xsecret_empty_input_is_empty_output() {
        let key = xsecret_key("token");
        let encrypted = xsecret_crypt(&key, 0, &[]).expect("encryption works");
        assert!(encrypted.is_empty());
    }
}
