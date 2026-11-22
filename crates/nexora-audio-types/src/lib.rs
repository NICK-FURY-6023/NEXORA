use std::fmt;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    Pcm,
    Opus,
    Flac,
    Aac,
    Mp3,
    Vorbis,
}

impl fmt::Display for AudioCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Pcm => "pcm",
            Self::Opus => "opus",
            Self::Flac => "flac",
            Self::Aac => "aac",
            Self::Mp3 => "mp3",
            Self::Vorbis => "vorbis",
        };
        f.write_str(text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub codec: AudioCodec,
}

impl AudioFormat {
    pub fn new(sample_rate: u32, channels: u16, codec: AudioCodec) -> Self {
        Self {
            sample_rate,
            channels,
            codec,
        }
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}ch/{}", self.sample_rate, self.channels, self.codec)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioFrame {
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl AudioFrame {
    pub fn new(
        sequence: u64,
        timestamp_ns: u64,
        sample_rate: u32,
        channels: u16,
        sample_count: usize,
    ) -> Self {
        let channels = channels.max(1);
        Self {
            sequence,
            timestamp_ns,
            sample_rate,
            channels,
            samples: vec![0.0; sample_count.saturating_mul(channels as usize)],
        }
    }

    pub fn sample_count(&self) -> usize {
        let channels = self.channels.max(1) as usize;
        self.samples.len() / channels
    }
}

/// Phase 1 contract for reusable frame buffers. No decoding or DSP here.
pub trait FramePool {
    fn acquire(
        &self,
        sequence: u64,
        timestamp_ns: u64,
        format: &AudioFormat,
        sample_count: usize,
    ) -> AudioFrame;

    fn release(&self, frame: AudioFrame);
}

#[derive(Debug, Default)]
pub struct VecFramePool {
    buffers: Mutex<Vec<Vec<f32>>>,
}

impl VecFramePool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pooled_buffer_count(&self) -> usize {
        self.buffers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

impl FramePool for VecFramePool {
    fn acquire(
        &self,
        sequence: u64,
        timestamp_ns: u64,
        format: &AudioFormat,
        sample_count: usize,
    ) -> AudioFrame {
        let needed = sample_count.saturating_mul(format.channels.max(1) as usize);
        let mut buffers = self
            .buffers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut samples = buffers.pop().unwrap_or_default();
        if samples.capacity() < needed {
            samples = Vec::with_capacity(needed);
        }
        samples.clear();
        samples.resize(needed, 0.0);
        AudioFrame {
            sequence,
            timestamp_ns,
            sample_rate: format.sample_rate,
            channels: format.channels,
            samples,
        }
    }

    fn release(&self, frame: AudioFrame) {
        let mut buffers = self
            .buffers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        buffers.push(frame.samples);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_and_format_roundtrip() {
        let codec = AudioCodec::Opus;
        let json = serde_json::to_string(&codec).unwrap();
        assert_eq!(json, "\"opus\"");
        let format = AudioFormat::new(48_000, 2, codec);
        assert_eq!(format.to_string(), "48000/2ch/opus");
    }

    #[test]
    fn frame_construction_and_sample_count() {
        let frame = AudioFrame::new(1, 100, 48_000, 2, 128);
        assert_eq!(frame.samples.len(), 256);
        assert_eq!(frame.sample_count(), 128);
    }

    #[test]
    fn pool_reuses_released_buffer() {
        let pool = VecFramePool::new();
        let format = AudioFormat::new(48_000, 2, AudioCodec::Pcm);
        let frame = pool.acquire(1, 0, &format, 64);
        assert_eq!(frame.samples.len(), 128);
        pool.release(frame);
        assert_eq!(pool.pooled_buffer_count(), 1);
        let reused = pool.acquire(2, 100, &format, 32);
        assert_eq!(pool.pooled_buffer_count(), 0);
        assert_eq!(reused.sequence, 2);
        assert_eq!(reused.sample_count(), 32);
    }
}
