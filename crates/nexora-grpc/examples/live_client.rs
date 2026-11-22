use nexora_grpc::nexora::session_service_client::SessionServiceClient;
use nexora_grpc::nexora::{AudioFormat, CreateSessionRequest, SessionRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = std::env::var("NEXORA_GRPC_ENDPOINT")
        .unwrap_or_else(|_| "http://127.0.0.1:9091".to_string());
    let mut client = SessionServiceClient::connect(endpoint).await?;
    let created = client
        .create_session(CreateSessionRequest {
            application: "live-smoke".to_string(),
            version: "1.0.0".to_string(),
            output: Some(AudioFormat {
                sample_rate: 48_000,
                channels: 2,
                codec: "opus".to_string(),
            }),
        })
        .await?
        .into_inner();
    let state = client
        .get_session(SessionRequest {
            session_id: created.session_id.clone(),
        })
        .await?
        .into_inner();
    println!(
        "session={} created_status={} live_status={}",
        created.session_id, created.status, state.status
    );
    Ok(())
}
