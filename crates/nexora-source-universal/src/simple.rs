use nexora_common::TrackId;
use nexora_source_api::{SourceError, TrackMetadata};
use serde_json::Value;

pub fn parse(body: &str, source_id: &str) -> Result<Vec<TrackMetadata>, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let items = root
        .get("data")
        .and_then(Value::as_array)
        .ok_or(SourceError::Unavailable)?;
    items
        .iter()
        .map(|item| {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .ok_or(SourceError::InvalidInput)?;
            let title = item
                .get("title")
                .and_then(Value::as_str)
                .ok_or(SourceError::InvalidInput)?;
            Ok(TrackMetadata {
                track_id: TrackId::new(id).map_err(|_| SourceError::InvalidInput)?,
                title: title.to_owned(),
                artist: item
                    .get("artist")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                duration_ms: None,
                source_id: source_id.to_owned(),
                available: true,
            })
        })
        .collect()
}
