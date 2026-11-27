//! Client-facing conformance smoke tests.
//!
//! These tests drive the shipped v4 surface from the outside, using the same
//! helpers as the compatibility harness. They pin the documented behavior of
//! partial surfaces rather than pretending a source or voice gateway exists.

use std::fs;

use serde_json::Value;

fn query_value(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D")
}

#[tokio::test]
async fn compat_client_a_seek_is_followed_by_a_player_update() {
    let root = crate::media_root();
    let (app, session_id) = crate::create_session_over_http(crate::app(&root)).await;
    let address = crate::serve(app.clone()).await;
    let mut stream = crate::handshake(&address, Some("1234"), None)
        .await
        .unwrap();

    let ready = crate::next_frame_skipping_metadata(&mut stream).await;
    assert_eq!(ready["op"], "ready");

    let loaded = crate::get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = loaded["data"]["tracks"][0]["encoded"]
        .as_str()
        .expect("encoded track")
        .to_owned();
    let (status, _) = crate::request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(&format!(r#"{{"track":{{"encoded":"{encoded}"}}}}"#)),
    )
    .await;
    assert_eq!(status, 200);

    assert_eq!(
        crate::next_frame_skipping_metadata(&mut stream).await["type"],
        "PlayerCreatedEvent"
    );
    assert_eq!(
        crate::next_frame_skipping_metadata(&mut stream).await["type"],
        "TrackStartEvent"
    );

    let (status, _) = crate::request(
        app.clone(),
        "PATCH",
        &format!("/nexora/sessions/{session_id}/players/1234"),
        Some(r#"{"position":50}"#),
    )
    .await;
    assert_eq!(status, 200);
    let seek = crate::next_frame_skipping_metadata(&mut stream).await;
    assert_eq!(seek["type"], "SeekEvent");
    assert_eq!(seek["position"], 50);

    let mut saw_update = false;
    for _ in 0..8 {
        let frame = crate::next_frame_skipping_metadata(&mut stream).await;
        if frame["op"] == "playerUpdate" {
            assert!(frame["state"]["position"].as_u64().unwrap() >= 50);
            assert_eq!(frame["state"]["connected"], false);
            assert_eq!(frame["state"]["ping"], -1);
            saw_update = true;
            break;
        }
    }
    assert!(saw_update, "a seek must be followed by a playerUpdate");

    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn compat_client_partial_surfaces_keep_their_documented_shapes() {
    let root = crate::media_root();
    let app = crate::app(&root);
    let loaded = crate::get_json(app.clone(), "/nexora/loadtracks?identifier=sample.wav").await;
    let encoded = query_value(
        loaded["data"]["tracks"][0]["encoded"]
            .as_str()
            .expect("encoded track"),
    );

    let lyrics = crate::get_json(
        app.clone(),
        &format!("/nexora/loadlyrics?encodedTrack={encoded}&source=local"),
    )
    .await;
    assert_eq!(lyrics["sourceName"], "local");
    assert_eq!(lyrics["type"], "none");
    assert!(lyrics["data"].is_null());

    let chapters = crate::get_json(
        app.clone(),
        &format!("/nexora/loadchapters?encodedTrack={encoded}"),
    )
    .await;
    assert!(chapters.is_array());
    assert_eq!(chapters.as_array().unwrap().len(), 0);

    let connection: Value = crate::get_json(app.clone(), "/nexora/connection").await;
    assert_eq!(connection["status"], "disabled");
    assert!(connection["metrics"].is_null());
    assert_eq!(
        connection["reason"],
        "connection_manager_unavailable_in_this_process"
    );

    let _ = fs::remove_dir_all(&root);
}
