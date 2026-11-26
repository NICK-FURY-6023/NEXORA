use nexora_source_api::{ResolvedSource, SourceError};
use nexora_source_universal::{
    deezer::{
        parse_search_response as deezer_search, require_token as deezer_token,
        resolve_details as deezer_resolve, safe_deezer_url,
    },
    oauth::TokenStore,
    tidal::{
        parse_search_response as tidal_search, require_token as tidal_token,
        resolve_details as tidal_resolve, safe_tidal_url,
    },
};

const DEEZER_SEARCH: &str = r#"{
  "data": [
    {
      "id": 123,
      "title": "Song",
      "artist": { "name": "Artist" },
      "duration": 180,
      "album": { "cover": "https://cdn.deezer.com/cover.jpg" },
      "preview": "https://cdnt-preview.deezer.com/preview.mp3"
    },
    { "id": 456, "title": "No preview", "artist": { "name": "Other" }, "duration": 0, "preview": null }
  ]
}"#;

const DEEZER_TRACK: &str = r#"{
  "id": 123,
  "title": "Song",
  "preview": "https://cdnt-preview.deezer.com/preview.mp3"
}"#;

const TIDAL_SEARCH: &str = r#"{
  "items": [
    {
      "id": 789,
      "title": "Track",
      "artist": { "name": "Performer" },
      "duration": 200,
      "album": { "cover": "https://resources.tidal.com/cover.jpg" }
    }
  ]
}"#;

const TIDAL_TRACK: &str = r#"{
  "id": 789,
  "title": "Track",
  "streamUrl": "https://media.tidal.com/audio/789"
}"#;

#[test]
fn deezer_search_maps_real_fields() {
    let tracks = deezer_search(DEEZER_SEARCH).expect("fixture should parse");
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].track_id.as_str(), "123");
    assert_eq!(tracks[0].title, "Song");
    assert_eq!(tracks[0].artist.as_deref(), Some("Artist"));
    assert_eq!(tracks[0].duration_ms, Some(180_000));
    assert_eq!(tracks[0].source_id, "deezer");
    assert_eq!(tracks[1].duration_ms, None);
}

#[test]
fn tidal_search_maps_real_fields() {
    let tracks = tidal_search(TIDAL_SEARCH).expect("fixture should parse");
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].track_id.as_str(), "789");
    assert_eq!(tracks[0].artist.as_deref(), Some("Performer"));
    assert_eq!(tracks[0].duration_ms, Some(200_000));
    assert_eq!(tracks[0].source_id, "tidal");
}

#[test]
fn deezer_resolve_returns_a_stream() {
    let source = deezer_resolve(DEEZER_TRACK).expect("preview exists");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://cdnt-preview.deezer.com/preview.mp3".to_owned()
        }
    );
}

#[test]
fn tidal_resolve_returns_a_stream() {
    let source = tidal_resolve(TIDAL_TRACK).expect("stream url exists");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://media.tidal.com/audio/789".to_owned()
        }
    );
}

#[test]
fn both_platforms_require_a_token() {
    let store = TokenStore::new();
    assert_eq!(deezer_token(&store), Err(SourceError::Unauthorized));
    assert_eq!(tidal_token(&store), Err(SourceError::Unauthorized));
}

#[test]
fn both_platforms_reject_unsafe_hosts() {
    assert!(safe_deezer_url("https://api.deezer.com/track/1").is_ok());
    assert!(safe_deezer_url("http://127.0.0.1/track/1").is_err());
    assert!(safe_tidal_url("https://api.tidal.com/v1/tracks/1").is_ok());
    assert!(safe_tidal_url("http://169.254.169.254/latest").is_err());
}
