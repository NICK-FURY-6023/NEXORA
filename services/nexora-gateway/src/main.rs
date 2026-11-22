use std::path::PathBuf;

use clap::Parser;
use nexora_common::ServiceName;
use nexora_config::NexoraConfig;
use nexora_error::NexoraResult;
use nexora_service::{HealthReport, Service, Shutdown, ShutdownReceiver, run_until_shutdown};
use nexora_telemetry::init_tracing;

#[derive(Debug, Parser)]
#[command(
    name = "nexora-gateway",
    version,
    about = "NEXORA gateway service skeleton"
)]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long)]
    log_filter: Option<String>,
}

pub struct GatewayService {
    config: NexoraConfig,
}

impl GatewayService {
    pub fn new(config: NexoraConfig) -> Self {
        Self { config }
    }
}

impl Service for GatewayService {
    fn name(&self) -> ServiceName {
        ServiceName::Gateway
    }

    fn health(&self) -> HealthReport {
        HealthReport::healthy(ServiceName::Gateway, "skeleton")
    }

    // Phase 1 skeleton only. REST, WebSocket and gRPC endpoints arrive in Phase 4.
    async fn run(&self, mut shutdown: ShutdownReceiver) -> NexoraResult<()> {
        tracing::info!(
            host = %self.config.server.host,
            port = self.config.server.port,
            "nexora gateway skeleton started"
        );
        shutdown.wait().await;
        tracing::info!("nexora gateway skeleton stopped gracefully");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> NexoraResult<()> {
    let args = Args::parse();
    let config = NexoraConfig::load(args.config.as_deref())?;
    let filter = args.log_filter.unwrap_or_else(|| "info".to_string());
    let _telemetry = init_tracing(&filter)?;

    let (shutdown, receiver) = Shutdown::new();
    let signal_shutdown = shutdown.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_shutdown.trigger();
        }
    });

    run_until_shutdown(GatewayService::new(config), receiver).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_health_is_healthy() {
        let service = GatewayService::new(NexoraConfig::default());
        let health = service.health();
        assert!(health.healthy);
        assert_eq!(health.service, ServiceName::Gateway);
    }
}
