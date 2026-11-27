use nexora_audio_types::{AudioCodec, AudioFormat};
use nexora_common::{SessionId, TrackId};
use nexora_protocol::PlayRequest;
use nexora_session::SessionManager;
use nexora_session::durable::{restore_snapshot, snapshot};

fn output() -> AudioFormat {
    AudioFormat::new(48_000, 2, AudioCodec::Opus)
}

#[tokio::test]
async fn a_stopped_session_is_not_restored() {
    let manager = SessionManager::new();
    let id = SessionId::new("durable-stopped").unwrap();
    manager.create_with_id(id.clone(), output()).await.unwrap();
    manager.stop(&id).await.unwrap();

    let restored = restore_snapshot(snapshot(&manager).await).unwrap();
    assert!(
        restored.get(&id).await.is_err(),
        "a stopped session must not come back after restart"
    );
}

#[tokio::test]
async fn a_restored_session_keeps_volume_and_queue_without_resuming_playback() {
    let manager = SessionManager::new();
    let id = SessionId::new("durable-live").unwrap();
    manager.create_with_id(id.clone(), output()).await.unwrap();
    manager.set_volume(&id, 80).await.unwrap();
    manager
        .enqueue(&id, TrackId::new("track-a").unwrap())
        .await
        .unwrap();

    let restored = restore_snapshot(snapshot(&manager).await).unwrap();
    let state = restored.get(&id).await.unwrap();
    assert_eq!(state.volume, 80);
    assert_eq!(restored.queue(&id).await.unwrap().len(), 1);
    assert_ne!(state.status, "playing");
}

#[tokio::test]
async fn a_playing_session_is_restored_paused_without_a_playback_task() {
    let manager = SessionManager::new();
    let id = SessionId::new("durable-playing").unwrap();
    manager.create_with_id(id.clone(), output()).await.unwrap();
    manager
        .play(
            &id,
            PlayRequest {
                track_id: TrackId::new("track-playing").unwrap(),
                start_position_ms: 250,
            },
        )
        .await
        .unwrap();

    let restored = restore_snapshot(snapshot(&manager).await).unwrap();
    let state = restored.get(&id).await.unwrap();
    assert_eq!(state.status, "paused");
    assert_eq!(state.position_ms, 250);
    assert_eq!(
        restored.current_track(&id).await.unwrap().unwrap().as_str(),
        "track-playing"
    );
}
