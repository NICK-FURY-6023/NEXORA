use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn writes_pcm16_wav_header_and_samples() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("nexora-wav-{unique}.wav"));
    let samples = vec![0.0f32, 0.5, -0.5, 1.0, -1.0];
    nexora_audio_codec::write_wav(&path, 48_000, 1, &samples).unwrap();
    let bytes = fs::read(&path).unwrap();
    let _ = fs::remove_file(&path);
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(bytes.len(), 44 + samples.len() * 2);
}

#[test]
fn opus_encoder_pads_a_partial_final_frame() {
    let samples = vec![0.1f32; 48_001];
    let packets = nexora_audio_codec::encode_opus_packets(48_000, 1, &samples).unwrap();
    assert_eq!(packets.len(), 51);
}

#[test]
fn encodes_opus_packets_and_writes_ogg_container() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("nexora-opus-{unique}.opus"));
    let samples: Vec<f32> = (0..9600)
        .map(|i| ((i as f32 / 9600.0) * std::f32::consts::TAU * 8.0).sin() * 0.2)
        .collect();
    let packets = nexora_audio_codec::encode_opus_packets(48_000, 1, &samples).unwrap();
    assert!(!packets.is_empty());
    assert!(packets.iter().any(|packet| !packet.is_empty()));

    nexora_audio_codec::write_ogg_opus(&path, 48_000, 1, &packets).unwrap();
    let bytes = fs::read(&path).unwrap();
    let _ = fs::remove_file(&path);
    assert_eq!(&bytes[0..4], b"OggS");
    assert!(bytes.windows(8).any(|window| window == b"OpusHead"));
    assert!(bytes.windows(8).any(|window| window == b"OpusTags"));
}
