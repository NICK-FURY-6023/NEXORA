use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use nexora_audio_pipeline::{OutputCodec, process_file};
use nexora_error::NexoraResult;
use nexora_telemetry::init_tracing;

#[derive(Debug, Parser)]
#[command(
    name = "nexora-audio-engine",
    version,
    about = "NEXORA standalone audio engine"
)]
struct Args {
    #[arg(long)]
    input: PathBuf,

    #[arg(long)]
    output: PathBuf,

    #[arg(long, default_value_t = 48_000)]
    target_rate: u32,

    #[arg(long, default_value_t = 2)]
    channels: u16,

    #[arg(long, value_enum, default_value_t = CodecArg::Pcm)]
    codec: CodecArg,

    #[arg(long, default_value = "info")]
    log_filter: String,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum CodecArg {
    Pcm,
    Opus,
}

fn main() -> NexoraResult<()> {
    let args = Args::parse();
    let _telemetry = init_tracing(&args.log_filter)?;

    let codec = match args.codec {
        CodecArg::Pcm => OutputCodec::Pcm,
        CodecArg::Opus => OutputCodec::Opus,
    };

    let started = Instant::now();
    let report = process_file(
        &args.input,
        &args.output,
        args.target_rate,
        args.channels,
        codec,
    )?;
    let elapsed = started.elapsed();

    let input_seconds = report.input_frames as f64 / report.input_rate.max(1) as f64;
    let realtime_factor = if elapsed.as_secs_f64() > 0.0 {
        input_seconds / elapsed.as_secs_f64()
    } else {
        0.0
    };

    tracing::info!(
        input_frames = report.input_frames,
        output_frames = report.output_frames,
        input_rate = report.input_rate,
        output_rate = report.output_rate,
        channels = report.channels,
        packets = report.packets,
        clipped = report.clipped,
        elapsed_ms = elapsed.as_millis() as u64,
        realtime_factor = format_args!("{realtime_factor:.2}x"),
        "nexora audio pipeline completed"
    );
    Ok(())
}
