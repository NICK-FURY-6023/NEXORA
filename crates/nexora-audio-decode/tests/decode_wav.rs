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
    for i in 0..frames {
        let value = ((i as f32 / frames as f32) * std::f32::consts::TAU).sin() * 0.5;
        data.extend_from_slice(&((value * 32767.0) as i16).to_le_bytes());
    }
    fs::write(path, data).unwrap();
}

#[test]
fn decodes_generated_wav_to_f32_samples() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("nexora-decode-{unique}.wav"));
    write_test_wav(&path, 44_100, 4096);

    let decoded = nexora_audio_decode::decode_file(&path).unwrap();
    let _ = fs::remove_file(&path);

    assert_eq!(decoded.format.sample_rate, 44_100);
    assert_eq!(decoded.format.channels, 1);
    assert!(decoded.samples.len() >= 4000);
    assert!(decoded.samples.iter().any(|sample| sample.abs() > 0.01));
}
