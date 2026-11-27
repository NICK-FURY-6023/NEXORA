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

fn token(variable: &str) -> String {
    std::env::var(variable).unwrap_or_else(|_| {
        panic!(
            "{variable} must contain the user's own authorized token; the test stays ignored otherwise"
        )
    })
}

fn tokens(provider: &str, variable: &str) -> Arc<TokenStore> {
    let mut store = TokenStore::new();
    store.put(provider, token(variable));
    Arc::new(store)
}

#[tokio::test]
#[ignore = "set NEXORA_LIVE_SPOTIFY_TOKEN and run with --ignored"]
async fn spotify_live_search_when_a_token_is_supplied() {
    let tokens = tokens("spotify", "NEXORA_LIVE_SPOTIFY_TOKEN");
    let results = SpotifySource::new(tokens)
        .search("nexora live smoke", &context())
        .await;
    assert!(
        results.is_ok(),
        "a valid user token should yield a search result"
    );
}

#[tokio::test]
#[ignore = "set NEXORA_LIVE_DEEZER_TOKEN and run with --ignored"]
async fn deezer_live_search_when_a_token_is_supplied() {
    let tokens = tokens("deezer", "NEXORA_LIVE_DEEZER_TOKEN");
    let results = DeezerSource::new(tokens)
        .search("nexora live smoke", &context())
        .await;
    assert!(
        results.is_ok(),
        "a valid user token should yield a search result"
    );
}

#[tokio::test]
#[ignore = "set NEXORA_LIVE_APPLE_TOKEN and run with --ignored"]
async fn apple_live_search_when_a_token_is_supplied() {
    let tokens = tokens("apple", "NEXORA_LIVE_APPLE_TOKEN");
    let results = AppleMusicSource::new(tokens)
        .search("nexora live smoke", &context())
        .await;
    assert!(
        results.is_ok(),
        "a valid user token should yield a search result"
    );
}

#[tokio::test]
#[ignore = "set NEXORA_LIVE_TIDAL_TOKEN and run with --ignored"]
async fn tidal_live_search_when_a_token_is_supplied() {
    let tokens = tokens("tidal", "NEXORA_LIVE_TIDAL_TOKEN");
    let results = TidalSource::new(tokens)
        .search("nexora live smoke", &context())
        .await;
    assert!(
        results.is_ok(),
        "a valid user token should yield a search result"
    );
}
