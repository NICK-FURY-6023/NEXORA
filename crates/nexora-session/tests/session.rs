use nexora_audio_types::{AudioCodec, AudioFormat};
use nexora_protocol::{PlayRequest, SessionEventType};
use nexora_session::SessionManager;

#[tokio::test]
async fn session_lifecycle_emits_events() {
    let manager = SessionManager::new();
    let mut events = manager.subscribe();
    let output = AudioFormat::new(48_000, 2, AudioCodec::Opus);
    let created = manager.create(output).await.unwrap();
    assert_eq!(created.status, "created");

    let session_id = nexora_common::SessionId::new(created.session_id.as_str()).unwrap();
    manager
        .play(
            &session_id,
            PlayRequest {
                track_id: nexora_common::TrackId::new("track_1").unwrap(),
                start_position_ms: 0,
            },
        )
        .await
        .unwrap();
    manager.pause(&session_id).await.unwrap();
    manager.resume(&session_id).await.unwrap();
    manager.set_volume(&session_id, 75).await.unwrap();

    let state = manager.get(&session_id).await.unwrap();
    assert_eq!(state.status, "playing");
    assert_eq!(state.volume, 75);

    let mut seen = Vec::new();
    for _ in 0..5 {
        seen.push(events.recv().await.unwrap().event_type);
    }
    assert!(seen.contains(&SessionEventType::SessionCreated));
    assert!(seen.contains(&SessionEventType::TrackStarted));
    assert!(seen.contains(&SessionEventType::PlaybackPaused));
    assert!(seen.contains(&SessionEventType::PlaybackResumed));
}

#[tokio::test]
async fn unknown_session_returns_error() {
    let manager = SessionManager::new();
    let id = nexora_common::SessionId::new("missing").unwrap();
    assert!(manager.get(&id).await.is_err());
}
