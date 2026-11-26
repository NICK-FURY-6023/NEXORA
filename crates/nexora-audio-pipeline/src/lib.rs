use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use audioadapter::{Adapter, AdapterMut};
use audioadapter_buffers::owned::InterleavedOwned;
use nexora_audio_codec::{encode_opus_packets, write_ogg_opus, write_wav};
use nexora_audio_decode::decode_file;
use nexora_error::{NexoraError, NexoraResult};
use nexora_protocol::nexora::NexoraFilters;
use rubato::{Fft, FixedSync, Resampler};
use tokio::sync::mpsc;

pub mod filters;
pub mod voice;

/// Interleaved sample-rate conversion using Rubato's FFT resampler.
pub fn resample_interleaved(
    input: &[f32],
    channels: usize,
    input_rate: u32,
    output_rate: u32,
) -> NexoraResult<Vec<f32>> {
    if channels == 0 {
        return Err(NexoraError::audio("channels must be greater than zero"));
    }
    if input.is_empty() || input_rate == output_rate {
        return Ok(input.to_vec());
    }
    if !input.len().is_multiple_of(channels) {
        return Err(NexoraError::audio(
            "input sample count must be divisible by channels",
        ));
    }

    let input_frames = input.len() / channels;
    let mut input_buffer = InterleavedOwned::<f32>::new(0.0, channels, input_frames);
    for frame in 0..input_frames {
        for channel in 0..channels {
            input_buffer.write_sample(channel, frame, &input[frame * channels + channel]);
        }
    }

    let mut resampler = Fft::<f32>::new(
        input_rate as usize,
        output_rate as usize,
        1024,
        channels,
        FixedSync::Both,
    )
    .map_err(|_error| NexoraError::audio("resampler initialization failed"))?;

    let output_buffer = resampler
        .process_all(&input_buffer, input_frames, None)
        .map_err(|_error| NexoraError::audio("resample failed"))?;

    let output_frames = output_buffer.frames();
    let mut output = Vec::with_capacity(output_frames * channels);
    for frame in 0..output_frames {
        for channel in 0..channels {
            let sample = output_buffer
                .read_sample(channel, frame)
                .ok_or_else(|| NexoraError::audio("resampler output index out of bounds"))?;
            output.push(sample);
        }
    }
    Ok(output)
}

/// True when any sample reaches or exceeds full scale.
pub fn detect_clipping(samples: &[f32]) -> bool {
    samples.iter().any(|sample| sample.abs() >= 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputCodec {
    Pcm,
    Opus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineReport {
    pub input_frames: usize,
    pub output_frames: usize,
    pub input_rate: u32,
    pub output_rate: u32,
    pub channels: u16,
    pub packets: usize,
    pub clipped: bool,
}

/// One 20 ms Opus packet ready for ordered delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpusFrame {
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub duration_ms: u16,
    pub payload: Vec<u8>,
}

impl OpusFrame {
    /// Build a frame with the standard 20 ms Opus cadence.
    pub fn packet(sequence: u64, payload: Vec<u8>) -> Self {
        Self {
            sequence,
            timestamp_ms: sequence.saturating_mul(20),
            duration_ms: 20,
            payload,
        }
    }
}

#[derive(Debug)]
struct PreparedPcm {
    samples: Vec<f32>,
    input_rate: u32,
    input_frames: usize,
    output_frames: usize,
    clipped: bool,
}

fn prepare_pcm(input: &Path, target_rate: u32, channels: u16) -> NexoraResult<PreparedPcm> {
    if channels == 0 {
        return Err(NexoraError::audio(
            "output channels must be greater than zero",
        ));
    }
    let decoded = decode_file(input)?;
    let input_channels = decoded.format.channels as usize;
    let input_frames = decoded.samples.len() / input_channels.max(1);
    let resampled = resample_interleaved(
        &decoded.samples,
        input_channels,
        decoded.format.sample_rate,
        target_rate,
    )?;
    let converted = remap_channels(&resampled, input_channels, channels as usize);
    let output_frames = converted.len() / channels as usize;
    Ok(PreparedPcm {
        clipped: detect_clipping(&converted),
        samples: converted,
        input_rate: decoded.format.sample_rate,
        input_frames,
        output_frames,
    })
}

/// Decode and encode a local file into 20 ms Opus packets with PCM gain.
///
/// Volume is applied before encoding so changes never mutate an Opus packet.
/// Values above 100 may clip and are clamped by the encoder input path.
pub fn encode_file_packets(
    input: &Path,
    target_rate: u32,
    channels: u16,
    volume: u8,
    filters: &NexoraFilters,
) -> NexoraResult<Vec<Vec<u8>>> {
    let mut prepared = prepare_pcm(input, target_rate, channels)?;
    if volume != 100 {
        let gain = (volume.min(150) as f32) / 100.0;
        for sample in &mut prepared.samples {
            *sample = (*sample * gain).clamp(-1.0, 1.0);
        }
    }
    filters::apply_filters(
        &mut prepared.samples,
        channels as usize,
        target_rate,
        filters,
    )?;
    encode_opus_packets(target_rate, channels, &prepared.samples)
}

/// Decode, resample, remap channels and write a standalone audio output file.
pub fn process_file(
    input: &Path,
    output: &Path,
    target_rate: u32,
    channels: u16,
    codec: OutputCodec,
) -> NexoraResult<PipelineReport> {
    let prepared = prepare_pcm(input, target_rate, channels)?;

    let mut packets = 0;
    match codec {
        OutputCodec::Pcm => write_wav(output, target_rate, channels, &prepared.samples)?,
        OutputCodec::Opus => {
            let encoded = encode_opus_packets(target_rate, channels, &prepared.samples)?;
            packets = encoded.len();
            write_ogg_opus(output, target_rate, channels, &encoded)?;
        }
    }

    Ok(PipelineReport {
        input_frames: prepared.input_frames,
        output_frames: prepared.output_frames,
        input_rate: prepared.input_rate,
        output_rate: target_rate,
        channels,
        packets,
        clipped: prepared.clipped,
    })
}

fn remap_channels(samples: &[f32], from: usize, to: usize) -> Vec<f32> {
    if from == 0 || to == 0 || samples.is_empty() {
        return Vec::new();
    }
    if from == to {
        return samples.to_vec();
    }
    let frames = samples.len() / from;
    let mut output = Vec::with_capacity(frames * to);
    for frame in 0..frames {
        let base = frame * from;
        let mono = if from == 1 {
            samples[base]
        } else {
            samples[base..base + from].iter().sum::<f32>() / from as f32
        };
        for _ in 0..to {
            output.push(mono);
        }
    }
    output
}

/// A bounded FIFO for interleaved audio frames.
#[derive(Debug, Clone)]
pub struct BoundedAudioQueue {
    sender: mpsc::Sender<Vec<f32>>,
    receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<Vec<f32>>>>,
    len: Arc<AtomicUsize>,
}

impl BoundedAudioQueue {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity.max(1));
        Self {
            sender,
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
            len: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn try_push(&self, samples: Vec<f32>) -> NexoraResult<()> {
        self.sender
            .try_send(samples)
            .map_err(|_| NexoraError::audio("audio queue is full"))?;
        self.len.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub async fn pop(&self) -> Option<Vec<f32>> {
        let samples = self.receiver.lock().await.recv().await;
        if samples.is_some() {
            self.len.fetch_sub(1, Ordering::SeqCst);
        }
        samples
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
