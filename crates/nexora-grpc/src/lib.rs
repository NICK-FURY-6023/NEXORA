use std::sync::Arc;

use nexora_audio_types::AudioCodec;
use nexora_common::{SessionId, TrackId};
use nexora_error::NexoraError;
use nexora_session::SessionManager;
use tonic::{Request, Response, Status};

pub mod nexora {
    include!(concat!(env!("OUT_DIR"), "/nexora.v1.rs"));
}

use nexora::session_service_server::SessionService;
use nexora::{
    CreateSessionRequest, CreateSessionResponse, Empty, PlayCommand, SessionRequest, SessionState,
};

#[derive(Debug)]
pub struct NexoraGrpcService {
    sessions: Arc<SessionManager>,
}

impl NexoraGrpcService {
    pub fn new(sessions: Arc<SessionManager>) -> Self {
        Self { sessions }
    }
}

fn invalid_session(error: NexoraError) -> Status {
    match error {
        NexoraError::Service { .. } => Status::not_found("session not found"),
        NexoraError::InvalidInput { .. } => Status::invalid_argument("invalid input"),
        _ => Status::internal("internal error"),
    }
}

fn parse_session_id(value: &str) -> Result<SessionId, Status> {
    SessionId::new(value).map_err(|_| Status::invalid_argument("invalid session id"))
}

fn codec_from_proto(codec: &str) -> AudioCodec {
    match codec.to_ascii_lowercase().as_str() {
        "pcm" => AudioCodec::Pcm,
        "flac" => AudioCodec::Flac,
        "aac" => AudioCodec::Aac,
        "mp3" => AudioCodec::Mp3,
        "vorbis" => AudioCodec::Vorbis,
        _ => AudioCodec::Opus,
    }
}

#[tonic::async_trait]
impl SessionService for NexoraGrpcService {
    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<CreateSessionResponse>, Status> {
        let request = request.into_inner();
        let output = request
            .output
            .ok_or_else(|| Status::invalid_argument("missing output format"))?;
        let format = nexora_audio_types::AudioFormat::new(
            output.sample_rate,
            output.channels as u16,
            codec_from_proto(&output.codec),
        );
        let created = self
            .sessions
            .create(format)
            .await
            .map_err(invalid_session)?;
        Ok(Response::new(CreateSessionResponse {
            session_id: created.session_id.as_str().to_string(),
            status: created.status,
            engine: created.engine,
            api_version: created.api_version.to_string(),
        }))
    }

    async fn get_session(
        &self,
        request: Request<SessionRequest>,
    ) -> Result<Response<SessionState>, Status> {
        let session_id = parse_session_id(&request.into_inner().session_id)?;
        let state = self
            .sessions
            .get(&session_id)
            .await
            .map_err(invalid_session)?;
        Ok(Response::new(SessionState {
            session_id: state.session_id.as_str().to_string(),
            status: state.status,
            position_ms: state.position_ms,
            volume: state.volume as u32,
        }))
    }

    async fn play(&self, request: Request<PlayCommand>) -> Result<Response<Empty>, Status> {
        let command = request.into_inner();
        let session_id = parse_session_id(&command.session_id)?;
        let track_id = TrackId::new(command.track_id)
            .map_err(|_| Status::invalid_argument("invalid track id"))?;
        self.sessions
            .play(
                &session_id,
                nexora_protocol::PlayRequest {
                    track_id,
                    start_position_ms: command.start_position_ms,
                },
            )
            .await
            .map_err(invalid_session)?;
        Ok(Response::new(Empty {}))
    }

    async fn pause(&self, request: Request<SessionRequest>) -> Result<Response<Empty>, Status> {
        let session_id = parse_session_id(&request.into_inner().session_id)?;
        self.sessions
            .pause(&session_id)
            .await
            .map_err(invalid_session)?;
        Ok(Response::new(Empty {}))
    }

    async fn stop(&self, request: Request<SessionRequest>) -> Result<Response<Empty>, Status> {
        let session_id = parse_session_id(&request.into_inner().session_id)?;
        self.sessions
            .stop(&session_id)
            .await
            .map_err(invalid_session)?;
        Ok(Response::new(Empty {}))
    }
}
