use nexora_source_api::SourceError;

pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Reads the host of an https URL the way the fetcher reads it.
///
/// The URL parser is used rather than string splitting: a backslash ends the
/// authority and a userinfo section precedes it, so a guard that reads the
/// authority by hand can be made to see an allowed host where the request
/// would go somewhere else entirely.
pub fn host_of(url: &str) -> Result<String, SourceError> {
    let host = host_of_any(url)?;
    if reqwest::Url::parse(url)
        .map_err(|_| SourceError::InvalidInput)?
        .scheme()
        != "https"
    {
        return Err(SourceError::InvalidInput);
    }
    Ok(host)
}

/// Reads the host of an http(s) URL the way the fetcher reads it.
pub fn host_of_any(url: &str) -> Result<String, SourceError> {
    let parsed = reqwest::Url::parse(url).map_err(|_| SourceError::InvalidInput)?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(SourceError::InvalidInput);
    }
    let host = parsed
        .host_str()
        .ok_or(SourceError::InvalidInput)?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() {
        return Err(SourceError::InvalidInput);
    }
    Ok(host)
}

pub async fn read_text(response: reqwest::Response) -> Result<String, SourceError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BODY_BYTES as u64)
    {
        return Err(SourceError::Unavailable);
    }
    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| SourceError::Unavailable)?
    {
        if body.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
            return Err(SourceError::Unavailable);
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).map_err(|_| SourceError::Unavailable)
}
