use nexora_source_api::SourceError;
use nexora_source_universal::{apple, deezer, oauth::TokenStore, tidal};

const FIXTURE: &str = r#"{
  "data": [
    { "id": "a1", "title": "One", "artist": "Artist A" },
    { "id": "a2", "title": "Two", "artist": "Artist B" }
  ]
}"#;

#[test]
fn deezer_requires_a_stored_token() {
    let store = TokenStore::new();
    assert_eq!(
        deezer::require_token(&store),
        Err(SourceError::Unauthorized)
    );
}

#[test]
fn apple_requires_a_stored_token() {
    let store = TokenStore::new();
    assert_eq!(apple::require_token(&store), Err(SourceError::Unauthorized));
}

#[test]
fn tidal_requires_a_stored_token() {
    let store = TokenStore::new();
    assert_eq!(tidal::require_token(&store), Err(SourceError::Unauthorized));
}

#[test]
fn deezer_search_maps_expected_shape() {
    let tracks = deezer::parse_search_response(FIXTURE).expect("fixture should parse");
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].track_id.as_str(), "a1");
    assert_eq!(tracks[0].title, "One");
    assert_eq!(tracks[0].artist.as_deref(), Some("Artist A"));
    assert_eq!(tracks[0].source_id, "deezer");
}

#[test]
fn apple_search_maps_expected_shape() {
    let tracks = apple::parse_search_response(FIXTURE).expect("fixture should parse");
    assert_eq!(tracks[0].source_id, "apple-music");
    assert_eq!(tracks[1].artist.as_deref(), Some("Artist B"));
}

#[test]
fn tidal_search_maps_expected_shape() {
    let tracks = tidal::parse_search_response(FIXTURE).expect("fixture should parse");
    assert_eq!(tracks[0].source_id, "tidal");
    assert_eq!(tracks[1].title, "Two");
}
