use nexora_audio_types::{AudioCodec, AudioFormat};
use nexora_protocol::{PlayRequest, SessionEventType};
use nexora_session::{PlaybackSignal, SessionManager};
use tokio::sync::{broadcast, watch};

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

#[tokio::test]
async fn attached_playback_receives_control_signals() {
    let manager = SessionManager::new();
    let created = manager
        .create(AudioFormat::new(48_000, 2, AudioCodec::Opus))
        .await
        .unwrap();
    let session_id = nexora_common::SessionId::new(created.session_id.as_str()).unwrap();
    let (sender, mut receiver) = watch::channel(PlaybackSignal::Resume);
    let (audio_out, _audio_receiver) = broadcast::channel(128);
    manager
        .attach_playback(&session_id, sender, audio_out)
        .await
        .unwrap();

    manager.pause(&session_id).await.unwrap();
    assert!(receiver.has_changed().unwrap());
    assert_eq!(*receiver.borrow_and_update(), PlaybackSignal::Pause);

    manager.resume(&session_id).await.unwrap();
    assert!(receiver.has_changed().unwrap());
    assert_eq!(*receiver.borrow_and_update(), PlaybackSignal::Resume);

    manager.seek(&session_id, 2_000).await.unwrap();
    assert!(receiver.has_changed().unwrap());
    assert_eq!(*receiver.borrow_and_update(), PlaybackSignal::Seek(2_000));

    manager.set_volume(&session_id, 40).await.unwrap();
    assert!(receiver.has_changed().unwrap());
    assert_eq!(
        *receiver.borrow_and_update(),
        PlaybackSignal::SetVolume(40)
    );

    manager.stop(&session_id).await.unwrap();
    assert!(receiver.has_changed().unwrap());
    assert_eq!(*receiver.borrow_and_update(), PlaybackSignal::Stop);
}

#[tokio::test]
async fn finishing_a_track_emits_track_ended() {
    let manager = SessionManager::new();
    let mut events = manager.subscribe();
    let created = manager
        .create(AudioFormat::new(48_000, 2, AudioCodec::Opus))
        .await
        .unwrap();
    let session_id = nexora_common::SessionId::new(created.session_id.as_str()).unwrap();
    manager.finish(&session_id).await.unwrap();

    let mut saw_end = false;
    for _ in 0..2 {
        let event = events.recv().await.unwrap();
        if event.session_id == session_id && event.event_type == SessionEventType::TrackEnded {
            saw_end = true;
            assert_eq!(event.payload["reason"], "finished");
        }
    }
    assert!(saw_end);
}

#[tokio::test]
async fn track_failed_emits_error() {
    let manager = SessionManager::new();
    let mut events = manager.subscribe();
    let created = manager
        .create(AudioFormat::new(48_000, 2, AudioCodec::Opus))
        .await
        .unwrap();
    let session_id = nexora_common::SessionId::new(created.session_id.as_str()).unwrap();
    manager.fail(&session_id, "source decode failed").await.unwrap();

    let mut saw_fail = false;
    for _ in 0..2 {
        let event = events.recv().await.unwrap();
        if event.session_id == session_id && event.event_type == SessionEventType::TrackFailed {
            saw_fail = true;
            assert_eq!(event.payload["reason"], "source decode failed");
        }
    }
    assert!(saw_fail);
    let state = manager.get(&session_id).await.unwrap();
    assert_eq!(state.status, "stopped");
}

#[tokio::test]
async fn queue_auto_advances_on_track_ended() {
    let manager = SessionManager::new();
    let mut events = manager.subscribe();
    let created = manager
        .create(AudioFormat::new(48_000, 2, AudioCodec::Opus))
        .await
        .unwrap();
    let session_id = nexora_common::SessionId::new(created.session_id.as_str()).unwrap();

    let track_a = nexora_common::TrackId::new("track_a").unwrap();
    let track_b = nexora_common::TrackId::new("track_b").unwrap();
    manager.enqueue(&session_id, track_a.clone()).await.unwrap();
    manager.enqueue(&session_id, track_b.clone()).await.unwrap();

    manager.play(&session_id, PlayRequest { track_id: track_a, start_position_ms: 0 }).await.unwrap();

    let state = manager.get(&session_id).await.unwrap();
    assert_eq!(state.status, "playing");

    manager.finish(&session_id).await.unwrap();

    // TrackEnded should be emitted for track_a.
    let mut saw_end = false;
    let mut saw_start = false;
    for _ in 0..4 {
        let event = events.recv().await.unwrap();
        if event.session_id == session_id {
            match event.event_type {
                SessionEventType::TrackEnded => saw_end = true,
                SessionEventType::TrackStarted => saw_start = true,
                _ => {}
            }
        }
    }
    assert!(saw_end);
    // Auto-advance should play track_b.
    assert!(saw_start);
    let state = manager.get(&session_id).await.unwrap();
    assert_eq!(state.status, "playing");
}

#[tokio::test]
async fn queue_does_not_auto_advance_when_empty() {
    let manager = SessionManager::new();
    let mut events = manager.subscribe();
    let created = manager
        .create(AudioFormat::new(48_000, 2, AudioCodec::Opus))
        .await
        .unwrap();
    let session_id = nexora_common::SessionId::new(created.session_id.as_str()).unwrap();

    manager.finish(&session_id).await.unwrap();

    let mut saw_end = false;
    let mut saw_start = false;
    for _ in 0..2 {
        let event = events.recv().await.unwrap();
        if event.session_id == session_id {
            match event.event_type {
                SessionEventType::TrackEnded => saw_end = true,
                SessionEventType::TrackStarted => saw_start = true,
                _ => {}
            }
        }
    }
    assert!(saw_end);
    assert!(!saw_start);
    let state = manager.get(&session_id).await.unwrap();
    assert_eq!(state.status, "stopped");
}
