//! Opt-in live-provider checks.
//!
//! These tests are ignored by default. Supply the user's own authorized token
//! in the matching environment variable and run with `--ignored` to exercise
//! the real provider endpoint. No token value is stored in the repository.

use std::sync::Arc;

use nexora_source_api::{AudioSource, AuthContext};
use nexora_source_universal::apple::AppleMusicSource;
use nexora_source_universal::deezer::DeezerSource;
use nexora_source_universal::oauth::TokenStore;
use nexora_source_universal::spotify::SpotifySource;
use nexora_source_universal::tidal::TidalSource;

fn context() -> AuthContext {
    AuthContext::new("nexora-live-tests", Vec::new()).expect("valid test application id")
}

fn tokens(provider: &str, variable: &str) -> Option<Arc<TokenStore>> {
    let token = std::env::var(variable)
        .ok()
        .filter(|value| !value.is_empty())?;
    let mut store = TokenStore::new();
    store.put(provider, token);
    Some(Arc::new(store))
}

#[tokio::test]
#[ignore = "set NEXORA_LIVE_SPOTIFY_TOKEN and run with --ignored"]
async fn spotify_live_search_when_a_token_is_supplied() {
    let Some(tokens) = tokens("spotify", "NEXORA_LIVE_SPOTIFY_TOKEN") else {
        eprintln!("skipping: NEXORA_LIVE_SPOTIFY_TOKEN is not set");
        return;
    };
    let results = SpotifySource::new(tokens)
        .search("nexora live smoke", &context())
        .await
        .expect("a valid user token should be accepted");
    assert!(
        !results.is_empty(),
        "search should return at least one track"
    );
}

#[tokio::test]
#[ignore = "set NEXORA_LIVE_DEEZER_TOKEN and run with --ignored"]
async fn deezer_live_search_when_a_token_is_supplied() {
    let Some(tokens) = tokens("deezer", "NEXORA_LIVE_DEEZER_TOKEN") else {
        eprintln!("skipping: NEXORA_LIVE_DEEZER_TOKEN is not set");
        return;
    };
    let results = DeezerSource::new(tokens)
        .search("nexora live smoke", &context())
        .await
        .expect("a valid user token should be accepted");
    assert!(
        !results.is_empty(),
        "search should return at least one track"
    );
}

#[tokio::test]
#[ignore = "set NEXORA_LIVE_APPLE_TOKEN and run with --ignored"]
async fn apple_live_search_when_a_token_is_supplied() {
    let Some(tokens) = tokens("apple", "NEXORA_LIVE_APPLE_TOKEN") else {
        eprintln!("skipping: NEXORA_LIVE_APPLE_TOKEN is not set");
        return;
    };
    let results = AppleMusicSource::new(tokens)
        .search("nexora live smoke", &context())
        .await
        .expect("a valid user token should be accepted");
    assert!(
        !results.is_empty(),
        "search should return at least one track"
    );
}

#[tokio::test]
#[ignore = "set NEXORA_LIVE_TIDAL_TOKEN and run with --ignored"]
async fn tidal_live_search_when_a_token_is_supplied() {
    let Some(tokens) = tokens("tidal", "NEXORA_LIVE_TIDAL_TOKEN") else {
        eprintln!("skipping: NEXORA_LIVE_TIDAL_TOKEN is not set");
        return;
    };
    let results = TidalSource::new(tokens)
        .search("nexora live smoke", &context())
        .await
        .expect("a valid user token should be accepted");
    assert!(
        !results.is_empty(),
        "search should return at least one track"
    );
}
