use std::sync::Once;

use nexora_error::{NexoraError, NexoraResult};
use tracing_subscriber::EnvFilter;

static INIT: Once = Once::new();

/// Guarantees that tracing is initialized at most once per process.
/// A second call is safe and still returns a guard.
pub fn init_tracing(filter: &str) -> NexoraResult<TelemetryGuard> {
    let env_filter = EnvFilter::try_new(filter)
        .map_err(|error| NexoraError::config(format!("invalid log filter: {error}")))?;

    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(false)
            .init();
    });

    Ok(TelemetryGuard)
}

#[derive(Debug, Clone)]
pub struct TelemetryGuard;

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        tracing::info!("telemetry shutdown");
    }
}

pub const METRIC_ACTIVE_SESSIONS: &str = "nexora_active_sessions";
pub const METRIC_AUDIO_FRAMES_TOTAL: &str = "nexora_audio_frames_total";
pub const METRIC_DECODE_ERRORS_TOTAL: &str = "nexora_decode_errors_total";
pub const METRIC_SOURCE_ERRORS_TOTAL: &str = "nexora_source_errors_total";
pub const METRIC_WORKER_CPU_USAGE: &str = "nexora_worker_cpu_usage";
pub const METRIC_WORKER_MEMORY_BYTES: &str = "nexora_worker_memory_bytes";
pub const METRIC_BUFFER_UNDERRUNS_TOTAL: &str = "nexora_buffer_underruns_total";
pub const METRIC_API_LATENCY_SECONDS: &str = "nexora_api_latency_seconds";
pub const METRIC_STREAM_BYTES_TOTAL: &str = "nexora_stream_bytes_total";
pub const METRIC_CACHE_HITS_TOTAL: &str = "nexora_cache_hits_total";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_is_idempotent() {
        let first = init_tracing("info").unwrap();
        let second = init_tracing("info").unwrap();
        drop(first);
        drop(second);
    }

    #[test]
    fn metric_names_are_stable() {
        assert_eq!(METRIC_ACTIVE_SESSIONS, "nexora_active_sessions");
        assert_eq!(METRIC_AUDIO_FRAMES_TOTAL, "nexora_audio_frames_total");
        assert_eq!(METRIC_CACHE_HITS_TOTAL, "nexora_cache_hits_total");
    }
}
