use nexora_common::TrackId;
use nexora_source_api::{ResolvedSource, SourceError, TrackMetadata};
use serde_json::Value;

use crate::oauth::TokenStore;

pub fn require_token(store: &TokenStore) -> Result<String, SourceError> {
    store.get("spotify").ok_or(SourceError::Unauthorized)
}

pub fn parse_search_response(body: &str) -> Result<Vec<TrackMetadata>, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let items = root
        .get("items")
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
                .get("name")
                .and_then(Value::as_str)
                .ok_or(SourceError::InvalidInput)?;
            Ok(TrackMetadata {
                track_id: TrackId::new(id).map_err(|_| SourceError::InvalidInput)?,
                title: title.to_owned(),
                artist: item
                    .pointer("/artists/0/name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                duration_ms: item
                    .get("duration_ms")
                    .and_then(Value::as_u64)
                    .filter(|ms| *ms > 0),
                source_id: "spotify".to_owned(),
                available: item.get("preview_url").and_then(Value::as_str).is_some(),
            })
        })
        .collect()
}

pub fn resolve_track(body: &str) -> Result<ResolvedSource, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let url = root
        .get("items")
        .and_then(Value::as_array)
        .ok_or(SourceError::Unavailable)?
        .iter()
        .filter_map(|item| item.get("preview_url").and_then(Value::as_str))
        .next()
        .ok_or(SourceError::NotFound)?;
    Ok(ResolvedSource::Stream {
        url: url.to_owned(),
    })
}
