use nexora_source_universal::detect;

#[test]
fn detects_archive_org_urls() {
    assert_eq!(
        detect("https://archive.org/details/test-item"),
        Some("internet-archive")
    );
}

#[test]
fn detects_bandcamp_urls() {
    assert_eq!(
        detect("https://artist.bandcamp.com/track/song"),
        Some("bandcamp-public")
    );
}

#[test]
fn detects_soundcloud_urls() {
    assert_eq!(
        detect("https://soundcloud.com/artist/song"),
        Some("soundcloud-public")
    );
}

#[test]
fn detects_youtube_urls() {
    assert_eq!(
        detect("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
        Some("youtube")
    );
}

#[test]
fn detects_spotify_urls() {
    assert_eq!(
        detect("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"),
        Some("spotify")
    );
}

#[test]
fn plain_query_has_no_platform() {
    assert_eq!(detect("hello world"), None);
}

#[test]
fn every_platform_id_is_reachable_from_detection() {
    use nexora_source_universal::PLATFORM_IDS;

    let urls = [
        "https://archive.org/details/x",
        "https://artist.bandcamp.com/track/x",
        "https://soundcloud.com/artist/x",
        "https://www.youtube.com/watch?v=abc",
        "https://open.spotify.com/track/abc",
        "https://www.deezer.com/track/123",
        "https://music.apple.com/us/album/x/123",
        "https://tidal.com/browse/track/123",
    ];
    for url in urls {
        let id = detect(url).unwrap_or_else(|| panic!("{url} must be detected"));
        assert!(PLATFORM_IDS.contains(&id), "unknown id {id}");
    }
}
