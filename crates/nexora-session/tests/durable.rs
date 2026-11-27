use nexora_audio_types::{AudioCodec, AudioFormat};
use nexora_common::{SessionId, TrackId};
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
