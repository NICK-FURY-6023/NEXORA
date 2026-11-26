use nexora_source_api::{ResolvedSource, SourceError};
use nexora_source_universal::{
    apple::{
        AppleMusicSource, parse_search_response, require_token, resolve_details, safe_apple_url,
    },
    oauth::TokenStore,
};

const APPLE_SEARCH: &str = r#"{
  "results": {
    "songs": {
      "data": [
        {
          "id": "1440857781",
          "type": "songs",
          "attributes": {
            "name": "Song",
            "artistName": "Artist",
            "durationInMillis": 200000,
            "previewUrl": "https://audio-ssl.itunes.apple.com/itunes-assets/preview.m4a"
          }
        },
        {
          "id": "1440857782",
          "type": "songs",
          "attributes": { "name": "No Preview", "artistName": "Other", "durationInMillis": 0 }
        }
      ]
    }
  }
}"#;

const APPLE_TRACK: &str = r#"{
  "data": [
    {
      "id": "1440857781",
      "type": "songs",
      "attributes": {
        "name": "Song",
        "artistName": "Artist",
        "durationInMillis": 200000,
        "previewUrl": "https://audio-ssl.itunes.apple.com/itunes-assets/preview.m4a"
      }
    }
  ]
}"#;

#[test]
fn apple_search_maps_catalog_fields() {
    let tracks = parse_search_response(APPLE_SEARCH).expect("fixture should parse");
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].track_id.as_str(), "1440857781");
    assert_eq!(tracks[0].title, "Song");
    assert_eq!(tracks[0].artist.as_deref(), Some("Artist"));
    assert_eq!(tracks[0].duration_ms, Some(200_000));
    assert_eq!(tracks[0].source_id, "apple-music");
    assert!(tracks[0].available);
    assert_eq!(tracks[1].duration_ms, None);
    assert!(!tracks[1].available);
}

#[test]
fn apple_resolve_returns_the_preview_stream() {
    let source = resolve_details(APPLE_TRACK).expect("preview url exists");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://audio-ssl.itunes.apple.com/itunes-assets/preview.m4a".to_owned()
        }
    );
}

#[test]
fn apple_resolve_without_preview_is_not_found() {
    let result = resolve_details(r#"{"data":[{"id":"1","attributes":{"name":"Song"}}]}"#);
    assert_eq!(result, Err(SourceError::NotFound));
}

#[test]
fn apple_requires_a_token() {
    let store = TokenStore::new();
    assert_eq!(require_token(&store), Err(SourceError::Unauthorized));
    let mut store = TokenStore::new();
    store.put("apple-music", "developer-token".to_owned());
    assert_eq!(require_token(&store).as_deref(), Ok("developer-token"));
}

#[test]
fn apple_hosts_are_allowlisted() {
    assert!(safe_apple_url("https://api.music.apple.com/v1/catalog/us/search").is_ok());
    assert!(safe_apple_url("https://music.apple.com/us/song/song/1440857781").is_ok());
    assert!(safe_apple_url("https://audio-ssl.itunes.apple.com/itunes-assets/preview.m4a").is_ok());
    for url in [
        "http://127.0.0.1/v1/catalog",
        "http://169.254.169.254/latest",
        "http://10.0.0.1/v1/catalog",
        "https://evil.example.org/v1/catalog",
        "ftp://api.music.apple.com/v1/catalog",
    ] {
        assert!(safe_apple_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn apple_hosts_cannot_be_smuggled_through_userinfo_or_a_query() {
    for url in [
        "https://audio-ssl.itunes.apple.com:443@evil.com/x",
        "https://api.music.apple.com@evil.com/x",
        "https://evil.com?@api.music.apple.com/x",
        "https://evil.com#@api.music.apple.com/x",
    ] {
        assert!(safe_apple_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn an_uppercase_host_is_still_the_allowed_host() {
    assert!(safe_apple_url("https://API.MUSIC.APPLE.COM/v1/catalog").is_ok());
}

#[test]
fn apple_source_has_a_stable_identity() {
    use nexora_source_api::{AudioSource, SourceCapabilities};

    let source = AppleMusicSource::new(std::sync::Arc::new(TokenStore::new()));
    assert_eq!(source.source_id(), "apple-music");
    assert_eq!(source.capabilities(), SourceCapabilities::full());
}
