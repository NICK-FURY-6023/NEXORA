use criterion::{Criterion, criterion_group, criterion_main};
use nexora_audio_pipeline::resample_interleaved;

fn benchmark_resample(c: &mut Criterion) {
    let samples: Vec<f32> = (0..8_192)
        .map(|index| ((index as f32 / 8_192.0) * std::f32::consts::TAU).sin() * 0.25)
        .collect();

    c.bench_function("resample_44100_to_48000_8192_frames", |bencher| {
        bencher.iter(|| {
            resample_interleaved(std::hint::black_box(&samples), 1, 44_100, 48_000).unwrap()
        });
    });
}

criterion_group!(benches, benchmark_resample);
criterion_main!(benches);
