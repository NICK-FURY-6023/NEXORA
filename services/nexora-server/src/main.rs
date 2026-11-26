use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use nexora_audio_types::AudioCodec;
use nexora_config::NexoraConfig;
use nexora_error::NexoraResult;
use nexora_grpc::NexoraGrpcService;
use nexora_grpc::nexora::session_service_server::SessionServiceServer;
use nexora_server::{AppState, RuntimeMetrics, build_router};
use nexora_session::{NexoraSessionManager, SessionManager};
use nexora_source_framework::{
    AuthorizedHttpAdapter, LocalMediaAdapter, PublicRadioAdapter, RadioStation, SourceRegistry,
};
use nexora_telemetry::init_tracing;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

#[derive(Debug, Parser)]
#[command(name = "nexora-server", version, about = "NEXORA hosted audio server")]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long)]
    host: Option<String>,

    #[arg(long)]
    port: Option<u16>,

    #[arg(long, default_value_t = 9090)]
    grpc_port: u16,

    #[arg(long)]
    password: Option<String>,

    #[arg(long)]
    media_root: Option<PathBuf>,

    #[arg(long, default_value = "info")]
    log_filter: String,
}

#[tokio::main]
async fn main() -> NexoraResult<()> {
    let args = Args::parse();
    let mut config = NexoraConfig::load(args.config.as_deref())?;
    if let Some(host) = args.host {
        config.server.host = host;
    }
    if let Some(port) = args.port {
        config.server.port = port;
    }
    let _telemetry = init_tracing(&args.log_filter)?;

    let sources = SourceRegistry::new();
    if let Some(root) = args.media_root.or_else(|| {
        let path = PathBuf::from("./data/media");
        path.is_dir().then_some(path)
    }) && let Ok(adapter) = LocalMediaAdapter::new(root)
    {
        let _ = sources.register(Box::new(adapter));
    }
    if let Ok(adapter) = PublicRadioAdapter::new(Vec::<RadioStation>::new()) {
        let _ = sources.register(Box::new(adapter));
    }
    let _ = sources.register(Box::new(AuthorizedHttpAdapter));

    let password = args
        .password
        .or_else(|| env::var("NEXORA_SERVER_PASSWORD").ok())
        .filter(|value| !value.is_empty());

    let sessions = Arc::new(SessionManager::new());
    let state = AppState {
        sessions: sessions.clone(),
        nexora_sessions: Arc::new(NexoraSessionManager::new()),
        sources: Arc::new(sources),
        password,
        metrics: Arc::new(RuntimeMetrics::default()),
        audio: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    };

    let grpc_address: SocketAddr = format!("{}:{}", config.server.host, args.grpc_port)
        .parse()
        .map_err(|_| nexora_error::NexoraError::invalid_input("invalid gRPC bind address"))?;
    let grpc_sessions = sessions.clone();
    tokio::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(grpc_address).await {
            Ok(listener) => listener,
            Err(error) => {
                tracing::error!(%error, %grpc_address, "cannot bind gRPC address");
                return;
            }
        };
        let service = NexoraGrpcService::new(grpc_sessions);
        if let Err(error) = Server::builder()
            .add_service(SessionServiceServer::new(service))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
        {
            tracing::error!(%error, "gRPC server failed");
        }
    });

    let address: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .map_err(|_| nexora_error::NexoraError::invalid_input("invalid bind address"))?;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| {
            nexora_error::NexoraError::io_with_source("cannot bind server address", error)
        })?;

    tracing::info!(%address, %grpc_address, codec = %AudioCodec::Opus, "nexora server listening");
    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|error| nexora_error::NexoraError::service_with_source("server failed", error))
}
