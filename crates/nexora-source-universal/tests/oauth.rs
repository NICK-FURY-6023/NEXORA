use nexora_source_universal::oauth::TokenStore;

#[test]
fn empty_store_has_no_token() {
    let store = TokenStore::new();
    assert_eq!(store.get("spotify"), None);
}

#[test]
fn stored_token_round_trips_per_provider() {
    let mut store = TokenStore::new();
    store.put("spotify", "token-a".to_owned());
    store.put("deezer", "token-b".to_owned());
    assert_eq!(store.get("spotify").as_deref(), Some("token-a"));
    assert_eq!(store.get("deezer").as_deref(), Some("token-b"));
    assert_eq!(store.get("tidal"), None);
}

#[test]
fn authorization_url_carries_a_pkce_challenge() {
    let store = TokenStore::new();
    let (url, verifier) = store
        .authorization_url("spotify", "client-1", "http://127.0.0.1/callback")
        .expect("url should build");
    assert!(url.contains("code_challenge="));
    assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%2Fcallback"));
    assert!(!url.contains(&verifier));
    assert_eq!(verifier.len(), 43);
}

#[test]
fn each_authorization_url_uses_a_new_verifier() {
    let store = TokenStore::new();
    let (_, first) = store
        .authorization_url("spotify", "client-1", "http://127.0.0.1/callback")
        .expect("first url");
    let (_, second) = store
        .authorization_url("spotify", "client-1", "http://127.0.0.1/callback")
        .expect("second url");
    assert_ne!(first, second);
}
