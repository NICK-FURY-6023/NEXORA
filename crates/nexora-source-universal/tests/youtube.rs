use nexora_source_api::{AudioSource, ResolvedSource, SourceCapabilities, SourceError};
use nexora_source_universal::youtube::{
    YouTubeSource, map_status, parse_search_response, player_body, resolve_details,
    safe_youtube_url, search_url, video_id,
};

const YOUTUBE_SEARCH: &str = r#"{
  "items": [
    {
      "id": { "videoId": "vid1" },
      "snippet": {
        "title": "Video",
        "channelTitle": "Channel",
        "duration": "3:45"
      }
    }
  ]
}"#;

const YOUTUBE_TRACK: &str = r#"{
  "streamingData": {
    "formats": [{ "url": "https://rr1---sn-abc.googlevideo.com/audio?sig=1", "mimeType": "audio/mp4" }]
  }
}"#;

#[test]
fn youtube_search_maps_video_id_and_channel() {
    let tracks = parse_search_response(YOUTUBE_SEARCH).expect("fixture should parse");
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].track_id.as_str(), "vid1");
    assert_eq!(tracks[0].title, "Video");
    assert_eq!(tracks[0].artist.as_deref(), Some("Channel"));
    assert_eq!(tracks[0].duration_ms, Some(225_000));
    assert_eq!(tracks[0].source_id, "youtube");
}

#[test]
fn youtube_resolve_returns_the_audio_stream() {
    let source = resolve_details(YOUTUBE_TRACK).expect("audio format exists");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://rr1---sn-abc.googlevideo.com/audio?sig=1".to_owned()
        }
    );
}

#[test]
fn youtube_resolve_rejects_a_stream_on_an_unsafe_host() {
    let body = r#"{
      "streamingData": {
        "formats": [{ "url": "http://127.0.0.1/audio", "mimeType": "audio/mp4" }]
      }
    }"#;
    assert_eq!(resolve_details(body), Err(SourceError::InvalidInput));
}

#[test]
fn youtube_video_id_is_taken_from_a_url_or_a_bare_id() {
    assert_eq!(video_id("vid1").as_deref(), Ok("vid1"));
    assert_eq!(
        video_id("https://www.youtube.com/watch?v=vid1").as_deref(),
        Ok("vid1")
    );
    assert_eq!(video_id("https://youtu.be/vid1").as_deref(), Ok("vid1"));
    assert_eq!(
        video_id("https://www.youtube.com/watch?v=vid1&t=42").as_deref(),
        Ok("vid1")
    );
}

#[test]
fn youtube_video_id_rejects_a_foreign_url_and_a_traversal() {
    assert_eq!(
        video_id("https://evil.example.com/watch?v=vid1"),
        Err(SourceError::InvalidInput)
    );
    assert_eq!(
        video_id("https://www.youtube.com/watch?v=../../etc/passwd"),
        Err(SourceError::InvalidInput)
    );
    assert_eq!(video_id(""), Err(SourceError::InvalidInput));
}

#[test]
fn youtube_search_targets_the_data_api_with_the_key() {
    let url = search_url("song", Some("key-123")).expect("url should build");
    assert_eq!(url.host_str(), Some("www.googleapis.com"));
    assert_eq!(url.path(), "/youtube/v3/search");
    let query: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert!(query.contains(&("q".to_owned(), "song".to_owned())));
    assert!(query.contains(&("key".to_owned(), "key-123".to_owned())));
}

#[test]
fn youtube_search_without_a_key_is_rejected() {
    assert_eq!(search_url("song", None), Err(SourceError::Unauthorized));
}

#[test]
fn youtube_player_body_carries_the_video_id() {
    let body = player_body("vid1").expect("body should build");
    assert_eq!(
        body,
        r#"{"context":{"client":{"clientName":"WEB","clientVersion":"2.20240101.00.00"}},"videoId":"vid1"}"#
    );
}

#[test]
fn youtube_hosts_cannot_be_smuggled_through_userinfo_or_a_query() {
    for url in [
        "https://www.youtube.com:443@evil.com/watch?v=vid1",
        "https://www.youtube.com@evil.com/watch?v=vid1",
        "https://evil.com?@www.youtube.com/watch?v=vid1",
        "https://evil.com#@www.youtube.com/watch?v=vid1",
    ] {
        assert!(safe_youtube_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn youtube_hosts_cannot_be_smuggled_through_a_backslash() {
    // A backslash ends the authority, so the host is what precedes it. The
    // guard must read the same host the fetcher will.
    for url in [
        r"https://evil.com\www.youtube.com/watch?v=vid1",
        r"https://evil.com\@www.youtube.com/watch?v=vid1",
    ] {
        assert!(safe_youtube_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn an_uppercase_youtube_host_is_still_the_allowed_host() {
    assert!(safe_youtube_url("https://WWW.YOUTUBE.COM/watch?v=vid1").is_ok());
}

#[test]
fn youtube_hosts_are_checked_before_use() {
    assert!(safe_youtube_url("https://www.youtube.com/watch?v=vid1").is_ok());
    assert!(safe_youtube_url("https://youtu.be/vid1").is_ok());
    assert!(safe_youtube_url("https://rr1---sn-abc.googlevideo.com/audio").is_ok());
    assert!(safe_youtube_url("https://evilyoutube.com/watch?v=vid1").is_err());
    assert!(safe_youtube_url("http://169.254.169.254/latest").is_err());
}

#[test]
fn provider_status_codes_map_to_source_errors() {
    assert_eq!(map_status(401), SourceError::Unauthorized);
    assert_eq!(map_status(429), SourceError::RateLimited);
    assert_eq!(map_status(503), SourceError::Unavailable);
}

#[test]
fn youtube_transport_source_has_a_stable_identity() {
    let source = YouTubeSource::new(None);
    assert_eq!(source.source_id(), "youtube");
    assert_eq!(source.capabilities(), SourceCapabilities::full());
}
