use std::path::PathBuf;

use nexora_source_api::{AuthContext, SourceCapabilities, TrackMetadata, TrackReference};

fn auth() -> AuthContext {
    AuthContext::new("my-bot", vec!["media.local".into(), "http.stream".into()]).unwrap()
}

#[test]
fn metadata_and_reference_contracts_are_constructible() {
    let track = TrackMetadata::local("track_1", "Song", PathBuf::from("/media/song.wav"));
    assert_eq!(track.source_id, "local-media");
    assert!(track.available);

    let reference = TrackReference::new("Song").with_source_hint("local-media");
    assert_eq!(reference.query, "Song");
    assert_eq!(reference.source_hint.as_deref(), Some("local-media"));
}

#[test]
fn auth_context_checks_scopes() {
    let ctx = auth();
    assert!(ctx.allows("media.local"));
    assert!(ctx.allows("http.stream"));
    assert!(!ctx.allows("radio.premium"));
    assert_eq!(ctx.application().as_str(), "my-bot");
}

#[test]
fn capabilities_are_stable() {
    let caps = SourceCapabilities::full();
    assert!(caps.search && caps.resolve && caps.health);
}
