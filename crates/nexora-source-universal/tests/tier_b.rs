use nexora_source_api::{ResolvedSource, SourceError};
use nexora_source_universal::{
    oauth::TokenStore,
    spotify::{parse_search_response, require_token, resolve_track},
    youtube::{map_status, parse_search_response as parse_youtube, resolve_video},
};

const SPOTIFY_FIXTURE: &str = r#"{
  "items": [
    {
      "id": "track-1",
      "name": "Song",
      "artists": [{ "name": "Artist" }],
      "duration_ms": 180000,
      "preview_url": "https://p.scdn.co/mp3-preview/abc"
    },
    { "id": "track-2", "name": "No preview", "artists": [], "duration_ms": 0, "preview_url": null }
  ]
}"#;

const YOUTUBE_FIXTURE: &str = r#"{
  "items": [
    { "id": { "videoId": "vid1" }, "snippet": { "title": "Video", "channelTitle": "Channel" } }
  ]
}"#;

const YOUTUBE_DETAILS: &str = r#"{
  "streamingData": {
    "formats": [{ "url": "https://media.example.com/audio?sig=1", "mimeType": "audio/mp4" }]
  }
}"#;

#[test]
fn spotify_search_maps_tracks() {
    let tracks = parse_search_response(SPOTIFY_FIXTURE).expect("fixture should parse");
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].title, "Song");
    assert_eq!(tracks[0].artist.as_deref(), Some("Artist"));
    assert_eq!(tracks[0].duration_ms, Some(180_000));
    assert_eq!(tracks[0].source_id, "spotify");
}

#[test]
fn spotify_missing_token_is_unauthorized() {
    let store = TokenStore::new();
    assert_eq!(require_token(&store), Err(SourceError::Unauthorized));
}

#[test]
fn spotify_stored_token_is_used() {
    let mut store = TokenStore::new();
    store.put("spotify", "token".to_owned());
    assert_eq!(require_token(&store).as_deref(), Ok("token"));
}

#[test]
fn spotify_resolve_uses_preview_url() {
    let source = resolve_track(SPOTIFY_FIXTURE).expect("first track has a preview");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://p.scdn.co/mp3-preview/abc".to_owned()
        }
    );
}

#[test]
fn youtube_search_maps_video_id_and_channel() {
    let tracks = parse_youtube(YOUTUBE_FIXTURE).expect("fixture should parse");
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].track_id.as_str(), "vid1");
    assert_eq!(tracks[0].title, "Video");
    assert_eq!(tracks[0].artist.as_deref(), Some("Channel"));
    assert_eq!(tracks[0].source_id, "youtube");
}

#[test]
fn youtube_resolve_uses_audio_format() {
    let source = resolve_video(YOUTUBE_DETAILS).expect("audio format exists");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://media.example.com/audio?sig=1".to_owned()
        }
    );
}

#[test]
fn provider_status_codes_map_to_source_errors() {
    assert_eq!(map_status(429), SourceError::RateLimited);
    assert_eq!(map_status(503), SourceError::Unavailable);
    assert_eq!(map_status(401), SourceError::Unauthorized);
}
