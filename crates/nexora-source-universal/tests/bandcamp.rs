use nexora_source_api::{ResolvedSource, SourceError};
use nexora_source_universal::bandcamp::{
    BandcampSource, parse_search_response, parse_tralbum_ids, resolve_details, safe_bandcamp_url,
};

const BANDCAMP_SEARCH: &str = r#"{
  "auto": {
    "results": [
      {
        "auto": {
          "band_id": 3494667229,
          "band_name": "Artist",
          "item_id": "2933339106",
          "item_type": "t",
          "item_title": "Song",
          "item_url": "https://artist.bandcamp.com/track/song"
        }
      }
    ]
  }
}"#;

const BANDCAMP_OEMBED: &str = r#"{
  "title": "Song",
  "author_name": "Artist",
  "author_url": "https://artist.bandcamp.com",
  "thumbnail_url": "https://f4.bcbits.com/img/cover.jpg",
  "html": "<script data-tralbum=\"{&quot;band_id&quot;:3494667229,&quot;tralbum_id&quot;:2933339106,&quot;tralbum_type&quot;:&quot;t&quot;,&quot;title&quot;:&quot;Song&quot;}\" src=\"https://bandcamp.com/EmbeddedPlayer.js\"></script>"
}"#;

const BANDCAMP_TRACK: &str = r#"{
  "id": 2933339106,
  "title": "Song",
  "artist": "Artist",
  "band": { "id": 3494667229, "name": "Artist" },
  "streaminfo": {
    "mp3-128": {
      "url": "https://t4.bcbits.com/stream/hash/mp3-128/2933339106?p=0&token=abc"
    }
  }
}"#;

#[test]
fn bandcamp_search_maps_track_rows() {
    let tracks = parse_search_response(BANDCAMP_SEARCH).expect("fixture should parse");
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].track_id.as_str(), "2933339106");
    assert_eq!(tracks[0].title, "Song");
    assert_eq!(tracks[0].artist.as_deref(), Some("Artist"));
    assert_eq!(tracks[0].source_id, "bandcamp-public");
}

#[test]
fn bandcamp_oembed_carries_the_tralbum_identity() {
    let ids = parse_tralbum_ids(BANDCAMP_OEMBED).expect("embed markup should parse");
    assert_eq!(ids.band_id, 3494667229);
    assert_eq!(ids.tralbum_id, 2933339106);
    assert_eq!(ids.tralbum_type, "t");
}

#[test]
fn bandcamp_resolve_returns_the_stream_url() {
    let source = resolve_details(BANDCAMP_TRACK).expect("stream info exists");
    assert_eq!(
        source,
        ResolvedSource::Stream {
            url: "https://t4.bcbits.com/stream/hash/mp3-128/2933339106?p=0&token=abc".to_owned()
        }
    );
}

#[test]
fn bandcamp_resolve_without_stream_info_is_not_found() {
    let result = resolve_details(r#"{"id":1,"title":"Song"}"#);
    assert_eq!(result, Err(SourceError::NotFound));
}

#[test]
fn bandcamp_resolve_rejects_a_stream_on_a_foreign_host() {
    let result = resolve_details(
        r#"{"streaminfo":{"mp3-128":{"url":"https://evil.example.org/stream.mp3"}}}"#,
    );
    assert_eq!(result, Err(SourceError::InvalidInput));
}

#[test]
fn bandcamp_hosts_are_allowlisted() {
    assert!(safe_bandcamp_url("https://artist.bandcamp.com/track/song").is_ok());
    assert!(safe_bandcamp_url("https://t4.bcbits.com/stream/hash/mp3-128/1").is_ok());
    for url in [
        "http://127.0.0.1/track/song",
        "http://169.254.169.254/latest",
        "http://10.0.0.1/track/song",
        "https://evil.example.com/?next=bandcamp.com",
        "ftp://bandcamp.com/track/song",
    ] {
        assert!(safe_bandcamp_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn bandcamp_hosts_cannot_be_smuggled_through_userinfo_or_a_query() {
    for url in [
        "https://artist.bandcamp.com:443@evil.com/track/song",
        "https://artist.bandcamp.com@evil.com/track/song",
        "https://evil.com?@artist.bandcamp.com/track/song",
        "https://evil.com#@artist.bandcamp.com/track/song",
    ] {
        assert!(safe_bandcamp_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn bandcamp_hosts_cannot_be_smuggled_through_a_backslash() {
    // A backslash ends the authority, so the host is what precedes it. The
    // guard must read the same host the fetcher will.
    for url in [
        r"https://evil.com\artist.bandcamp.com/track/song",
        r"https://evil.com\@artist.bandcamp.com/track/song",
    ] {
        assert!(safe_bandcamp_url(url).is_err(), "{url} must be rejected");
    }
}

#[test]
fn an_uppercase_host_is_still_the_allowed_host() {
    assert!(safe_bandcamp_url("https://ARTIST.BANDCAMP.COM/track/song").is_ok());
}

#[test]
fn bandcamp_source_has_a_stable_identity() {
    use nexora_source_api::{AudioSource, SourceCapabilities};

    let source = BandcampSource::new();
    assert_eq!(source.source_id(), "bandcamp-public");
    assert_eq!(source.capabilities(), SourceCapabilities::full());
}
