use nexora_source_api::ResolvedSource;
use nexora_source_universal::{
    archive::{parse_search_response, resolve_details, safe_archive_url},
    bandcamp::{parse_oembed as parse_bandcamp_oembed, safe_bandcamp_url},
    soundcloud::{parse_oembed as parse_soundcloud_oembed, safe_soundcloud_url},
};

const SEARCH_FIXTURE: &str = r#"{
  "response": {
    "docs": [
      {
        "identifier": "test-item",
        "title": "Test Track",
        "creator": ["Test Artist"],
        "length": 123
      },
      {
        "identifier": "no-duration",
        "title": "No Duration",
        "creator": "Solo Artist"
      }
    ]
  }
}"#;

const DETAILS_FIXTURE: &str = r#"{
  "response": {
    "metadata": { "identifier": "test-item" },
    "files": [
      { "name": "test-item_flac.flac", "format": "Flac" },
      { "name": "test-item.mp3", "format": "VBR MP3" },
      { "name": "__ia_thumb.jpg", "format": "JPEG" }
    ]
  }
}"#;

#[test]
fn search_maps_duration_and_artist() {
    let tracks = parse_search_response(SEARCH_FIXTURE).expect("fixture should parse");
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].title, "Test Track");
    assert_eq!(tracks[0].artist.as_deref(), Some("Test Artist"));
    assert_eq!(tracks[0].duration_ms, Some(123_000));
    assert_eq!(tracks[0].source_id, "internet-archive");
    assert_eq!(tracks[1].duration_ms, None);
    assert_eq!(tracks[1].artist.as_deref(), Some("Solo Artist"));
}

#[test]
fn empty_search_returns_no_tracks() {
    let tracks = parse_search_response(r#"{"response":{"docs":[]}}"#).expect("valid JSON");
    assert!(tracks.is_empty());
}

#[test]
fn resolve_picks_the_first_audio_file() {
    let source = resolve_details(DETAILS_FIXTURE).expect("audio file exists");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://archive.org/download/test-item/test-item_flac.flac".to_owned()
        }
    );
}

#[test]
fn resolve_without_audio_reports_not_found() {
    let result = resolve_details(
        r#"{"response":{"metadata":{"identifier":"x"},"files":[{"name":"a.jpg","format":"JPEG"}]}}"#,
    );
    assert!(result.is_err());
}

#[test]
fn unsafe_hosts_are_rejected() {
    for url in [
        "http://127.0.0.1/details/x",
        "http://169.254.169.254/latest/meta-data",
        "http://10.0.0.1/details/x",
        "https://evil.example.org/details/x",
        "ftp://archive.org/details/x",
    ] {
        assert!(safe_archive_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn archive_org_https_url_is_accepted() {
    assert!(safe_archive_url("https://archive.org/details/test-item").is_ok());
}

const BANDCAMP_OEMBED: &str = r#"{
  "title": "Song",
  "author_name": "Artist",
  "author_url": "https://artist.bandcamp.com",
  "thumbnail_url": "https://f4.bcbits.com/img/cover.jpg"
}"#;

const SOUNDCLOUD_OEMBED: &str = r#"{
  "title": "Track",
  "author_name": "Publisher",
  "author_url": "https://soundcloud.com/publisher",
  "thumbnail_url": "https://i1.sndcdn.com/artworks-cover.jpg"
}"#;

#[test]
fn bandcamp_oembed_maps_to_track_metadata() {
    let track = parse_bandcamp_oembed(BANDCAMP_OEMBED).expect("fixture should parse");
    assert_eq!(track.title, "Song");
    assert_eq!(track.artist.as_deref(), Some("Artist"));
    assert_eq!(track.duration_ms, None);
    assert_eq!(track.source_id, "bandcamp-public");
}

#[test]
fn soundcloud_oembed_maps_to_track_metadata() {
    let track = parse_soundcloud_oembed(SOUNDCLOUD_OEMBED).expect("fixture should parse");
    assert_eq!(track.title, "Track");
    assert_eq!(track.artist.as_deref(), Some("Publisher"));
    assert_eq!(track.source_id, "soundcloud-public");
}

#[test]
fn platform_hosts_are_checked_before_use() {
    assert!(safe_bandcamp_url("https://artist.bandcamp.com/track/song").is_ok());
    assert!(safe_bandcamp_url("https://evil.example.com/?next=bandcamp.com").is_err());
    assert!(safe_soundcloud_url("https://soundcloud.com/artist/song").is_ok());
    assert!(safe_soundcloud_url("http://127.0.0.1/artist/song").is_err());
}
