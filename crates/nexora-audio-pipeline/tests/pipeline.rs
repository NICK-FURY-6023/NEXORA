use nexora_audio_pipeline::{BoundedAudioQueue, detect_clipping, resample_interleaved};

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
