use nexora_source_api::{AudioSource, ResolvedSource, SourceCapabilities, SourceError};
use nexora_source_universal::{
    oauth::TokenStore,
    spotify::{
        SpotifySource, parse_search_response, require_token, resolve_details, safe_spotify_url,
        track_id,
    },
};

const SPOTIFY_SEARCH: &str = r#"{
  "tracks": {
    "items": [
      {
        "id": "abc123",
        "name": "Song",
        "artists": [{ "name": "Artist" }],
        "duration_ms": 180000,
        "preview_url": "https://p.scdn.co/mp3-preview/abc"
      },
      { "id": "def456", "name": "No preview", "artists": [], "duration_ms": 0, "preview_url": null }
    ]
  }
}"#;

const SPOTIFY_TRACK: &str = r#"{
  "id": "abc123",
  "name": "Song",
  "preview_url": "https://p.scdn.co/mp3-preview/abc"
}"#;

#[test]
fn spotify_search_maps_real_tracks_envelope() {
    let tracks = parse_search_response(SPOTIFY_SEARCH).expect("fixture should parse");
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].track_id.as_str(), "abc123");
    assert_eq!(tracks[0].title, "Song");
    assert_eq!(tracks[0].artist.as_deref(), Some("Artist"));
    assert_eq!(tracks[0].duration_ms, Some(180_000));
    assert_eq!(tracks[0].source_id, "spotify");
    assert!(tracks[0].available);
    assert!(!tracks[1].available);
}

#[test]
fn spotify_resolve_returns_the_preview_stream() {
    let source = resolve_details(SPOTIFY_TRACK).expect("preview exists");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://p.scdn.co/mp3-preview/abc".to_owned()
        }
    );
}

#[test]
fn spotify_resolve_rejects_a_preview_url_on_an_unsafe_host() {
    let body = r#"{ "id": "x", "preview_url": "http://169.254.169.254/latest/meta-data" }"#;
    assert_eq!(resolve_details(body), Err(SourceError::InvalidInput));
}

#[test]
fn spotify_requires_a_stored_token() {
    let store = TokenStore::new();
    assert_eq!(require_token(&store), Err(SourceError::Unauthorized));
}

#[test]
fn spotify_track_id_is_taken_from_a_url_or_a_bare_id() {
    assert_eq!(track_id("abc123").as_deref(), Ok("abc123"));
    assert_eq!(
        track_id("https://open.spotify.com/track/abc123").as_deref(),
        Ok("abc123")
    );
    assert_eq!(
        track_id("https://open.spotify.com/track/abc123?si=xyz").as_deref(),
        Ok("abc123")
    );
}

#[test]
fn spotify_track_id_rejects_a_foreign_url_and_a_traversal() {
    assert_eq!(
        track_id("https://evil.example.com/track/abc123"),
        Err(SourceError::InvalidInput)
    );
    assert_eq!(
        track_id("https://open.spotify.com/track/../../etc/passwd"),
        Err(SourceError::InvalidInput)
    );
    assert_eq!(track_id(""), Err(SourceError::InvalidInput));
}

#[test]
fn spotify_hosts_cannot_be_smuggled_through_userinfo_or_a_query() {
    for url in [
        "https://open.spotify.com:443@evil.com/track/abc123",
        "https://open.spotify.com@evil.com/track/abc123",
        "https://evil.com?@open.spotify.com/track/abc123",
        "https://evil.com#@open.spotify.com/track/abc123",
    ] {
        assert!(safe_spotify_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn an_uppercase_spotify_host_is_still_the_allowed_host() {
    assert!(safe_spotify_url("https://OPEN.SPOTIFY.COM/track/abc123").is_ok());
}

#[test]
fn spotify_hosts_are_checked_before_use() {
    assert!(safe_spotify_url("https://open.spotify.com/track/abc123").is_ok());
    assert!(safe_spotify_url("https://api.spotify.com/v1/tracks/abc123").is_ok());
    assert!(safe_spotify_url("https://p.scdn.co/mp3-preview/abc").is_ok());
    assert!(safe_spotify_url("https://evilspotify.com/track/abc123").is_err());
    assert!(safe_spotify_url("http://127.0.0.1/track/abc123").is_err());
    assert!(safe_spotify_url("https://evil.example.com/?next=spotify.com").is_err());
}

#[test]
fn spotify_transport_source_has_a_stable_identity() {
    let source = SpotifySource::new(std::sync::Arc::new(TokenStore::new()));
    assert_eq!(source.source_id(), "spotify");
    assert_eq!(source.capabilities(), SourceCapabilities::full());
}
