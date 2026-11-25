use nexora_common::TrackId;
use nexora_source_api::{ResolvedSource, SourceError, TrackMetadata};
use serde_json::Value;

pub fn map_status(status: u16) -> SourceError {
    match status {
        401 | 403 => SourceError::Unauthorized,
        429 => SourceError::RateLimited,
        _ => SourceError::Unavailable,
    }
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
                .pointer("/id/videoId")
                .and_then(Value::as_str)
                .ok_or(SourceError::InvalidInput)?;
            Ok(TrackMetadata {
                track_id: TrackId::new(id).map_err(|_| SourceError::InvalidInput)?,
                title: item
                    .pointer("/snippet/title")
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_owned(),
                artist: item
                    .pointer("/snippet/channelTitle")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                duration_ms: None,
                source_id: "youtube".to_owned(),
                available: true,
            })
        })
        .collect()
}

pub fn resolve_video(body: &str) -> Result<ResolvedSource, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let url = root
        .pointer("/streamingData/formats")
        .and_then(Value::as_array)
        .ok_or(SourceError::Unavailable)?
        .iter()
        .filter_map(|format| format.get("url").and_then(Value::as_str))
        .next()
        .ok_or(SourceError::NotFound)?;
    Ok(ResolvedSource::Stream {
        url: url.to_owned(),
    })
}
