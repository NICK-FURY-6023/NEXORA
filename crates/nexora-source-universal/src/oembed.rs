use nexora_common::TrackId;
use nexora_source_api::{SourceError, TrackMetadata};
use serde_json::Value;

pub fn parse(body: &str, source_id: &str) -> Result<TrackMetadata, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let title = root
        .get("title")
        .and_then(Value::as_str)
        .ok_or(SourceError::InvalidInput)?;
    let identity = root
        .get("author_url")
        .and_then(Value::as_str)
        .unwrap_or(title);
    Ok(TrackMetadata {
        track_id: TrackId::new(identity).map_err(|_| SourceError::InvalidInput)?,
        title: title.to_owned(),
        artist: root
            .get("author_name")
            .and_then(Value::as_str)
            .map(str::to_owned),
        duration_ms: None,
        source_id: source_id.to_owned(),
        available: true,
    })
}
