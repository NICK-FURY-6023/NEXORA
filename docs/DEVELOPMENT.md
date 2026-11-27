# NEXORA Development

## Toolchain

Rust 1.100.0 is pinned in `rust-toolchain.toml`. Components: `rustfmt`, `clippy`. Edition: 2024.

## Baseline commands

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Current baseline at commit `e6ae4eb`: all four commands pass; `cargo test
--workspace` reports 367 tests, 0 failures. CI runs the same gates on Linux
and Windows (green run `45995404326`).

## Running the gateway skeleton

```bash
cargo run -p nexora-gateway -- --log-filter debug
```

Optional config file:

```bash
cargo run -p nexora-gateway -- --config ./config.toml
```

The Phase 1 gateway is a skeleton: it loads configuration, starts tracing, waits for Ctrl-C, and shuts down gracefully. It does not open HTTP/WebSocket/gRPC listeners yet.

## Running the Phase 2 audio engine

```bash
cargo run -p nexora-audio-engine -- \
  --input ./input.wav \
  --output ./output.opus \
  --target-rate 48000 \
  --channels 2 \
  --codec opus \
  --log-filter info
```

`--codec pcm` writes a 16-bit PCM WAV file. `--codec opus` writes an Ogg Opus file. The engine reports frames, elapsed time, and measured real-time factor.

## Benchmark

```bash
cargo bench -p nexora-audio-pipeline --bench audio_pipeline
```

Use Criterion flags such as `--warm-up-time 0.1 --measurement-time 0.2 --sample-size 10` for a quick noisy check. Document hardware, versions, input, and settings before treating any number as a performance result.

## Configuration precedence

1. Built-in defaults in `nexora-config`.
2. Optional TOML file passed with `--config`.
3. Explicit environment variables such as `NEXORA_SERVER__PORT` and `NEXORA_AUDIO__SAMPLE_RATE`.

Invalid values are rejected during `NexoraConfig::validate()`.

## Dependency policy

Direct dependencies are pinned with exact versions in the root `Cargo.toml`; transitive dependencies are controlled by the committed `Cargo.lock`. Add a dependency only with a technical reason, license check, MSRV check, and phase justification.
