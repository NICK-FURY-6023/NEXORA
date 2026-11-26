use nexora_source_api::{ResolvedSource, SourceError};
use nexora_source_universal::{
    oauth::TokenStore,
    soundcloud::{
        SoundcloudSource, parse_search_response, require_client_id, resolve_details,
        safe_soundcloud_url,
    },
};

const SOUNDCLOUD_SEARCH: &str = r#"{
  "collection": [
    {
      "id": 123456,
      "title": "Song",
      "duration": 200000,
      "permalink_url": "https://soundcloud.com/artist/song",
      "user": { "username": "Artist" },
      "media": {
        "transcodings": [
          { "format": { "protocol": "hls" }, "url": "https://api-v2.soundcloud.com/media/soundcloud:tracks:123456/a/stream/hls" }
        ]
      }
    },
    {
      "id": 654321,
      "title": "No Media",
      "duration": 0,
      "user": { "username": "Other" },
      "media": { "transcodings": [] }
    }
  ]
}"#;

const SOUNDCLOUD_TRACK: &str = r#"{
  "id": 123456,
  "title": "Song",
  "duration": 200000,
  "user": { "username": "Artist" },
  "media": {
    "transcodings": [
      { "format": { "protocol": "hls" }, "url": "https://api-v2.soundcloud.com/media/soundcloud:tracks:123456/a/stream/hls" },
      { "format": { "protocol": "progressive" }, "url": "https://api-v2.soundcloud.com/media/soundcloud:tracks:123456/b/stream/progressive" }
    ]
  }
}"#;

#[test]
fn soundcloud_search_maps_collection_rows() {
    let tracks = parse_search_response(SOUNDCLOUD_SEARCH).expect("fixture should parse");
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].track_id.as_str(), "123456");
    assert_eq!(tracks[0].title, "Song");
    assert_eq!(tracks[0].artist.as_deref(), Some("Artist"));
    assert_eq!(tracks[0].duration_ms, Some(200_000));
    assert_eq!(tracks[0].source_id, "soundcloud-public");
    assert!(tracks[0].available);
    assert!(!tracks[1].available);
}

#[test]
fn soundcloud_resolve_prefers_the_progressive_transcoding() {
    let source = resolve_details(SOUNDCLOUD_TRACK, "client-id").expect("transcoding exists");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://api-v2.soundcloud.com/media/soundcloud:tracks:123456/b/stream/progressive?client_id=client-id"
                .to_owned()
        }
    );
}

#[test]
fn soundcloud_resolve_without_media_is_not_found() {
    let result = resolve_details(r#"{"id":1,"title":"Song"}"#, "client-id");
    assert_eq!(result, Err(SourceError::NotFound));
}

#[test]
fn soundcloud_requires_a_client_id() {
    let store = TokenStore::new();
    assert_eq!(require_client_id(&store), Err(SourceError::Unauthorized));
    let mut store = TokenStore::new();
    store.put("soundcloud", "client-id".to_owned());
    assert_eq!(require_client_id(&store).as_deref(), Ok("client-id"));
}

#[test]
fn soundcloud_hosts_are_allowlisted() {
    assert!(safe_soundcloud_url("https://soundcloud.com/artist/song").is_ok());
    assert!(safe_soundcloud_url("https://api-v2.soundcloud.com/tracks/1").is_ok());
    for url in [
        "http://127.0.0.1/artist/song",
        "http://169.254.169.254/latest",
        "http://10.0.0.1/artist/song",
        "https://evil.example.org/artist/song",
        "ftp://soundcloud.com/artist/song",
    ] {
        assert!(safe_soundcloud_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn soundcloud_source_has_a_stable_identity() {
    use nexora_source_api::{AudioSource, SourceCapabilities};

    let source = SoundcloudSource::new(std::sync::Arc::new(TokenStore::new()));
    assert_eq!(source.source_id(), "soundcloud-public");
    assert_eq!(source.capabilities(), SourceCapabilities::full());
}
