use nexora_source_api::SourceError;

pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
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
