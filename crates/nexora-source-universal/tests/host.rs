use nexora_source_universal::http::host_of;

#[test]
fn the_host_is_read_the_way_a_fetcher_reads_it() {
    assert_eq!(
        host_of("https://api.spotify.com/v1/tracks").as_deref(),
        Ok("api.spotify.com")
    );
    assert_eq!(
        host_of("https://OPEN.SPOTIFY.COM/v1").as_deref(),
        Ok("open.spotify.com")
    );
    assert_eq!(
        host_of("https://api.spotify.com:443/v1").as_deref(),
        Ok("api.spotify.com")
    );
    assert_eq!(
        host_of("https://api.spotify.com./v1").as_deref(),
        Ok("api.spotify.com")
    );
}

#[test]
fn a_smuggled_url_reports_the_host_the_request_would_reach() {
    // A backslash ends the authority and a userinfo section precedes it, so
    // the host is not the one a guard reading the authority by hand sees.
    // Reporting the true host is what lets the allow list reject it.
    assert_eq!(
        host_of(r"https://evil.com\api.spotify.com/v1").as_deref(),
        Ok("evil.com")
    );
    assert_eq!(
        host_of(r"https://evil.com\@api.spotify.com/v1").as_deref(),
        Ok("evil.com")
    );
    assert_eq!(
        host_of("https://api.spotify.com@evil.com/v1").as_deref(),
        Ok("evil.com")
    );
}

#[test]
fn only_https_urls_have_a_host() {
    assert!(host_of("http://api.spotify.com/v1").is_err());
    assert!(host_of("file:///etc/passwd").is_err());
    assert!(host_of("api.spotify.com").is_err());
    assert!(host_of("").is_err());
}
