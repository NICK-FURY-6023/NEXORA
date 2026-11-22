use nexora_grpc::nexora::session_service_client::SessionServiceClient;
use nexora_grpc::nexora::{AudioFormat, CreateSessionRequest, PlayCommand, SessionRequest};
use nexora_session::SessionManager;
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::test]
async fn grpc_session_lifecycle_works() {
    let sessions = Arc::new(SessionManager::new());
    let service = nexora_grpc::NexoraGrpcService::new(sessions.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        Server::builder()
            .add_service(
                nexora_grpc::nexora::session_service_server::SessionServiceServer::new(service),
            )
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });

    let mut client = SessionServiceClient::connect(format!("http://{address}"))
        .await
        .unwrap();
    let created = client
        .create_session(CreateSessionRequest {
            application: "test-bot".into(),
            version: "1.0.0".into(),
            output: Some(AudioFormat {
                sample_rate: 48_000,
                channels: 2,
                codec: "opus".into(),
            }),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(created.status, "created");

    client
        .play(PlayCommand {
            session_id: created.session_id.clone(),
            track_id: "track_1".into(),
            start_position_ms: 0,
        })
        .await
        .unwrap();

    let state = client
        .get_session(SessionRequest {
            session_id: created.session_id,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(state.status, "playing");
}
