use criterion::{Criterion, criterion_group, criterion_main};
use nexora_audio_pipeline::{filters::apply_filters, resample_interleaved};
use nexora_protocol::nexora::{NexoraEqualizerBand, NexoraFilters};

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

fn benchmark_filters(c: &mut Criterion) {
    let samples: Vec<f32> = (0..16_384)
        .map(|index| ((index as f32 / 128.0) * std::f32::consts::TAU).sin() * 0.25)
        .collect();
    let filters = NexoraFilters {
        volume: Some(0.9),
        equalizer: Some(vec![
            NexoraEqualizerBand { band: 0, gain: 3.0 },
            NexoraEqualizerBand {
                band: 4,
                gain: -3.0,
            },
        ]),
        ..NexoraFilters::default()
    };

    c.bench_function("filters_volume_and_two_eq_bands_8192_frames", |bencher| {
        bencher.iter(|| {
            let mut output = samples.clone();
            apply_filters(
                std::hint::black_box(&mut output),
                2,
                48_000,
                std::hint::black_box(&filters),
            )
            .unwrap();
        });
    });
}

criterion_group!(benches, benchmark_resample, benchmark_filters);
criterion_main!(benches);
