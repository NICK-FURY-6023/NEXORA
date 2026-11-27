//! Every provider fetch goes through the shared client and the shared reader.
//!
//! The shared client carries the request timeout and the shared reader caps
//! the response body. An adapter that builds its own client or reads the body
//! itself loses both bounds, so the rule is checked here rather than left to
//! review.

/// The adapter that owns each provider's HTTP calls.
const ADAPTERS: &[(&str, &str)] = &[
    ("apple", include_str!("../src/apple.rs")),
    ("bandcamp", include_str!("../src/bandcamp.rs")),
    ("deezer", include_str!("../src/deezer.rs")),
    ("soundcloud", include_str!("../src/soundcloud.rs")),
    ("spotify", include_str!("../src/spotify.rs")),
    ("tidal", include_str!("../src/tidal.rs")),
    ("transport", include_str!("../src/transport.rs")),
    ("youtube", include_str!("../src/youtube.rs")),
];

#[test]
fn every_adapter_fetches_through_the_shared_client() {
    for (name, source) in ADAPTERS {
        assert!(
            source.contains("crate::http::client()"),
            "{name} must use the shared client, which carries the request timeout"
        );
        assert!(
            !source.contains("reqwest::Client::new()"),
            "{name} must not build its own client: that client has no timeout"
        );
    }
}

#[test]
fn every_adapter_reads_the_body_through_the_shared_reader() {
    for (name, source) in ADAPTERS {
        assert!(
            source.contains("crate::http::read_text("),
            "{name} must read the body through the shared reader, which caps its size"
        );
        assert!(
            !source.contains(".text().await"),
            "{name} must not read the body itself: that read has no size cap"
        );
    }
}
