use nexora_common::TrackId;
use nexora_source_api::{ResolvedSource, SourceError, TrackMetadata};
use serde_json::Value;

const SOURCE_ID: &str = "internet-archive";
const AUDIO_EXTENSIONS: [&str; 6] = [".flac", ".mp3", ".ogg", ".m4a", ".wav", ".opus"];

pub fn safe_archive_url(url: &str) -> Result<&str, SourceError> {
    let path = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or(SourceError::InvalidInput)?;
    let host = path
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    if host == "archive.org" || host == "www.archive.org" {
        Ok(url)
    } else {
        Err(SourceError::InvalidInput)
    }
}

pub fn parse_search_response(body: &str) -> Result<Vec<TrackMetadata>, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let docs = root
        .pointer("/response/docs")
        .and_then(Value::as_array)
        .ok_or(SourceError::Unavailable)?;

    docs.iter()
        .map(|doc| {
            let identifier = doc
                .get("identifier")
                .and_then(Value::as_str)
                .ok_or(SourceError::InvalidInput)?;
            let title = doc
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(identifier)
                .to_owned();
            let artist = doc.get("creator").and_then(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| value.as_array()?.first()?.as_str().map(str::to_owned))
            });
            let duration_ms = doc
                .get("length")
                .and_then(Value::as_u64)
                .map(|seconds| seconds * 1_000);
            Ok(TrackMetadata {
                track_id: TrackId::new(identifier).map_err(|_| SourceError::InvalidInput)?,
                title,
                artist,
                duration_ms,
                source_id: SOURCE_ID.to_owned(),
                available: true,
            })
        })
        .collect()
}

pub fn resolve_details(body: &str) -> Result<ResolvedSource, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let identifier = root
        .pointer("/response/metadata/identifier")
        .and_then(Value::as_str)
        .ok_or(SourceError::InvalidInput)?;
    let name = root
        .pointer("/response/files")
        .and_then(Value::as_array)
        .ok_or(SourceError::Unavailable)?
        .iter()
        .filter_map(|file| file.get("name").and_then(Value::as_str))
        .find(|name| {
            let lower = name.to_ascii_lowercase();
            AUDIO_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
        })
        .ok_or(SourceError::NotFound)?;
    Ok(ResolvedSource::Stream {
        url: format!("https://archive.org/download/{identifier}/{name}"),
    })
}
