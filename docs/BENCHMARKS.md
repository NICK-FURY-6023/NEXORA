# NEXORA Benchmarks

This is a reproducible measurement record, not a production performance
claim. The numbers below come from a real `cargo bench` run on this host; they
are specific to its CPU, toolchain and process load.

## Environment

| Item | Value |
|---|---|
| Date | 2026-11-27 |
| Commit | `ed22756` (benchmark harness worktree) |
| OS | Windows 11, MSYS shell |
| CPU | Intel Core i3-1005G1 @ 1.20 GHz, 4 logical processors reported by `nproc` |
| Rust | `rustc 1.100.0 (0ef1bc386 2026-11-09)` |
| Cargo | `cargo 1.100.0 (3acc79c0c 2026-11-05)` |
| Criterion | `0.8.2`, exact-pinned in the workspace manifest |
| Profile | `--bench` uses Cargo's optimized `bench` profile |
| Plot backend | Plotters (gnuplot not installed; does not affect timing) |

## Command

```bash
cargo bench -p nexora-audio-pipeline --bench audio_pipeline
```

Criterion warmed up for 3 s per benchmark, then collected 100 samples. The
resample benchmark ran 20,000 iterations per sample in an estimated 6.03 s
measurement window; the filter benchmark ran 45,000 iterations per sample in an
estimated 5.05 s window. `std::hint::black_box` is applied to inputs.

## Results

Values are nanoseconds per operation. The headline is Criterion's **slope**
estimate with its 95% confidence interval; mean and median are included because
this host showed outliers.

| Benchmark | Slope (95% CI) | Mean (95% CI) | Median (95% CI) | Outliers |
|---|---|---|---|---|
| `resample_44100_to_48000_8192_frames` | 298,701 (285,702–315,466) | 310,921 (297,535–325,273) | 286,266 (272,723–302,529) | 3/100 high mild |
| `filters_volume_and_two_eq_bands_8192_frames` | 103,375 (99,713–108,522) | 104,703 (101,219–109,948) | 100,271 (99,343–101,906) | 10/100 (5 mild, 5 severe) |

The first benchmark resamples 8,192 mono f32 frames from 44.1 kHz to 48 kHz.
The second applies volume and two equalizer bands to 8,192 stereo frames
(16,384 interleaved f32 samples) at 48 kHz.

## Run-to-run variance

The resample benchmark was measured twice on the same tree. Its slope estimate
was 276,293 ns in the first run and 298,701 ns in the second, with overlapping
confidence intervals. Treat this spread as host noise; do not use a delta this
small as a regression signal without a quieter, controlled machine and more
repetitions.

## Interpretation

- These are microbenchmarks of two hot paths in `nexora-audio-pipeline`; they do
  not measure end-to-end server latency, WebSocket delivery, or voice quality.
- The filter run's high outliers are consistent with scheduler noise on a
  4-logical-processor development host. The median is reported alongside the
  mean so that the outliers are visible.
- The server-level harness below adds an in-process HTTP workload, but it is
  still one host and one session-create route, not a distributed capacity
  claim.

## Server-level HTTP workload

Same host and toolchain as above. The harness starts a fresh in-process Axum
router and issues 100 `POST /nexora/sessions` requests per iteration through
`tower::ServiceExt::oneshot`, asserting each response is 201. Warm-up was 3 s,
100 samples were collected in an estimated 7.22 s measurement window, and each
sample ran 15,000 iterations of the 100-request batch.

```bash
cargo bench -p nexora-server --bench server_load
```

| Benchmark | Slope (95% CI) | Mean (95% CI) | Median (95% CI) | Outliers |
|---|---|---|---|---|
| `create_100_sessions_through_the_router` | 462,785 ns (456,881–471,064) | 463,592 ns (459,187–468,834) | 458,114 ns (455,321–459,430) | 10/100 (4 mild, 6 severe) |

This is a per-batch measurement: divide by 100 for an approximate per-request
cost only as a rough indicator. It does not include network sockets, WebSocket
audio delivery, disk persistence, or concurrent clients, so it is not a
production capacity figure.

## Reproduce

1. Run the exact command above on a quiet machine.
2. Read `target/criterion/<benchmark>/new/estimates.json` for the same fields
   used here: `slope`, `mean`, `median`, and `std_dev`.
3. Record the toolchain, host, commit and command with the numbers, as done
   above.
