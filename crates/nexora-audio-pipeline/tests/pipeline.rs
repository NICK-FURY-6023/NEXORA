use nexora_audio_pipeline::{
    BoundedAudioQueue, detect_clipping, encode_file_packets, resample_interleaved,
};
use nexora_protocol::v4::V4Filters;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn write_test_wav(path: &PathBuf, sample_rate: u32, frames: usize) {
    let mut data = Vec::with_capacity(44 + frames * 2);
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + frames as u32 * 2).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&(frames as u32 * 2).to_le_bytes());
    for index in 0..frames {
        let value = ((index as f32 / frames as f32) * std::f32::consts::TAU).sin() * 0.5;
        data.extend_from_slice(&((value * 32767.0) as i16).to_le_bytes());
    }
    fs::write(path, data).unwrap();
}

fn temp_wav(name: &str, sample_rate: u32, frames: usize) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("nexora-{name}-{unique}.wav"));
    write_test_wav(&path, sample_rate, frames);
    path
}

#[test]
fn resamples_interleaved_audio_to_target_rate() {
    let input: Vec<f32> = (0..44_100)
        .map(|i| (i as f32 / 44_100.0).sin() * 0.1)
        .collect();
    let output = resample_interleaved(&input, 1, 44_100, 48_000).unwrap();
    assert!(output.len() > 44_100);
    assert!(output.len() < 50_000);
}

#[test]
fn detects_clipping_samples() {
    assert!(detect_clipping(&[0.0, 0.5, 1.0, -0.5]));
    assert!(!detect_clipping(&[0.0, 0.5, -0.5]));
}

#[tokio::test]
async fn bounded_queue_enforces_capacity() {
    let queue = BoundedAudioQueue::new(2);
    assert!(queue.try_push(vec![1.0]).is_ok());
    assert!(queue.try_push(vec![2.0]).is_ok());
    assert!(queue.try_push(vec![3.0]).is_err());
    assert_eq!(queue.len(), 2);
}

#[test]
fn encode_file_packets_produces_20ms_opus_packets() {
    let path = temp_wav("packets", 44_100, 8_820);
    let packets = encode_file_packets(&path, 48_000, 2, 100, &V4Filters::default()).unwrap();
    let _ = fs::remove_file(&path);

    assert!((9..=11).contains(&packets.len()));
    assert!(packets.iter().all(|packet| !packet.is_empty()));
}

#[test]
fn encode_file_packets_applies_volume_gain() {
    let path = temp_wav("volume", 44_100, 4_410);
    let loud = encode_file_packets(&path, 48_000, 2, 100, &V4Filters::default()).unwrap();
    let silent = encode_file_packets(&path, 48_000, 2, 0, &V4Filters::default()).unwrap();
    let _ = fs::remove_file(&path);

    assert_eq!(loud.len(), silent.len());
    assert_ne!(loud[0], silent[0]);
}
