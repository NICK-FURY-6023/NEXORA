use std::fs::{self, File};
use std::path::Path;

use nexora_error::{NexoraError, NexoraResult};
use ogg::writing::{PacketWriteEndInfo, PacketWriter};
use opus_rs::{Application, OpusEncoder};

const OGG_SERIAL: u32 = 0x4E58_4F52;

/// Write interleaved f32 samples as a 16-bit PCM WAV file.
pub fn write_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    samples: &[f32],
) -> NexoraResult<()> {
    if channels == 0 {
        return Err(NexoraError::audio("channels must be greater than zero"));
    }
    let data_len = samples.len().saturating_mul(2);
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36u32.wrapping_add(data_len as u32)).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.wrapping_mul(channels as u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&channels.wrapping_mul(2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());

    for sample in samples {
        let clipped = sample.clamp(-1.0, 1.0);
        let pcm = (clipped * 32_767.0) as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }

    fs::write(path, bytes)
        .map_err(|error| NexoraError::io_with_source("cannot write wav file", error))
}

/// Encode interleaved f32 PCM into 20 ms Opus packets.
pub fn encode_opus_packets(
    sample_rate: u32,
    channels: u16,
    samples: &[f32],
) -> NexoraResult<Vec<Vec<u8>>> {
    if channels == 0 {
        return Err(NexoraError::audio("channels must be greater than zero"));
    }
    let mut encoder = OpusEncoder::new(sample_rate as i32, channels as usize, Application::Audio)
        .map_err(|_error| NexoraError::audio("opus encoder initialization failed"))?;
    let frame_samples = (sample_rate as usize / 50).max(1);
    let chunk_samples = frame_samples * channels as usize;
    let mut output = vec![0u8; 4_000];
    let mut packets = Vec::new();

    let mut padded = vec![0.0f32; chunk_samples];
    for chunk in samples.chunks(chunk_samples) {
        let input = if chunk.len() == chunk_samples {
            chunk
        } else {
            padded[..chunk.len()].copy_from_slice(chunk);
            padded[chunk.len()..].fill(0.0);
            &padded[..]
        };
        let written = encoder
            .encode(input, frame_samples, &mut output)
            .map_err(|_error| NexoraError::audio("opus encoding failed"))?;
        packets.push(output[..written].to_vec());
    }

    Ok(packets)
}

/// Write Opus packets into a minimal Ogg Opus container.
pub fn write_ogg_opus(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    packets: &[Vec<u8>],
) -> NexoraResult<()> {
    if channels == 0 {
        return Err(NexoraError::audio("channels must be greater than zero"));
    }
    let file = File::create(path)
        .map_err(|error| NexoraError::io_with_source("cannot create ogg opus file", error))?;
    let mut writer = PacketWriter::new(file);
    let pre_skip = opus_lookahead(sample_rate);
    let frame_samples = (sample_rate as usize / 50).max(1) as u64;

    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1);
    head.push(channels as u8);
    head.extend_from_slice(&(pre_skip as u16).to_le_bytes());
    head.extend_from_slice(&sample_rate.to_le_bytes());
    head.extend_from_slice(&0i16.to_le_bytes());
    head.push(0);
    writer
        .write_packet(&head[..], OGG_SERIAL, PacketWriteEndInfo::EndPage, 0)
        .map_err(|error| NexoraError::audio_with_source("ogg head write failed", error))?;

    let vendor = b"NEXORA";
    let mut tags = Vec::with_capacity(8 + 4 + vendor.len() + 4);
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes());
    writer
        .write_packet(&tags[..], OGG_SERIAL, PacketWriteEndInfo::NormalPacket, 0)
        .map_err(|error| NexoraError::audio_with_source("ogg tags write failed", error))?;

    let mut granule = pre_skip as u64;
    for (index, packet) in packets.iter().enumerate() {
        granule = granule.saturating_add(frame_samples);
        let info = if index + 1 == packets.len() {
            PacketWriteEndInfo::EndStream
        } else {
            PacketWriteEndInfo::NormalPacket
        };
        writer
            .write_packet(&packet[..], OGG_SERIAL, info, granule)
            .map_err(|error| NexoraError::audio_with_source("ogg packet write failed", error))?;
    }

    writer
        .into_inner()
        .sync_all()
        .map_err(|error| NexoraError::io_with_source("cannot sync ogg opus file", error))
}

fn opus_lookahead(sample_rate: u32) -> usize {
    (sample_rate as usize / 400) + (sample_rate as usize / 250)
}
