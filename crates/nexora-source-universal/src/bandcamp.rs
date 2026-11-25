use nexora_source_api::{SourceError, TrackMetadata};

use crate::oembed;

pub fn parse_oembed(body: &str) -> Result<TrackMetadata, SourceError> {
    oembed::parse(body, "bandcamp-public")
}

pub fn safe_bandcamp_url(url: &str) -> Result<&str, SourceError> {
    let path = url
        .strip_prefix("https://")
        .ok_or(SourceError::InvalidInput)?;
    let host = path.split('/').next().unwrap_or("");
    if host == "bandcamp.com" || host.ends_with(".bandcamp.com") {
        Ok(url)
    } else {
        Err(SourceError::InvalidInput)
    }
}
