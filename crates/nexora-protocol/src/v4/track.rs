use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use nexora_error::NexoraError;

/// Track metadata of the Lavalink v4 track shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct V4TrackInfo {
    pub identifier: String,
    #[serde(rename = "isSeekable")]
    pub is_seekable: bool,
    pub author: String,
    pub length: u64,
    #[serde(rename = "isStream")]
    pub is_stream: bool,
    pub position: u64,
    pub title: String,
    pub uri: Option<String>,
    #[serde(rename = "artworkUrl")]
    pub artwork_url: Option<String>,
    pub isrc: Option<String>,
    #[serde(rename = "sourceName")]
    pub source_name: String,
}

/// A Lavalink v4 track: the encoded handle plus its metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct V4Track {
    pub encoded: String,
    pub info: V4TrackInfo,
    #[serde(rename = "pluginInfo")]
    pub plugin_info: Value,
    #[serde(rename = "userData")]
    pub user_data: Value,
}

/// Encode a track into NEXORA's opaque handle.
///
/// The payload keys stay `track_id`, `source_id`, `title` and `duration_ms`
/// so handles written by earlier builds keep decoding.
pub fn encode_track(info: &V4TrackInfo, source_payload: &Value) -> Result<String, NexoraError> {
    let mut payload = json!({
        "track_id": info.identifier,
        "source_id": info.source_name,
        "title": info.title,
        "duration_ms": info.length,
        "artist": info.author,
    });
    if let Some(target) = payload.as_object_mut() {
        if let Some(uri) = &info.uri {
            target.insert("uri".to_owned(), json!(uri));
        }
        if let Some(artwork_url) = &info.artwork_url {
            target.insert("artwork_url".to_owned(), json!(artwork_url));
        }
        if let Some(isrc) = &info.isrc {
            target.insert("isrc".to_owned(), json!(isrc));
        }
    }
    let merged = merge_source_payload(payload, source_payload);
    let bytes = serde_json::to_vec(&merged)
        .map_err(|error| NexoraError::protocol(format!("track encoding failed: {error}")))?;
    Ok(STANDARD.encode(bytes))
}

/// Decode an opaque handle back into track metadata.
pub fn decode_track(encoded: &str) -> Result<V4TrackInfo, NexoraError> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| NexoraError::invalid_input(format!("invalid encoded track: {error}")))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| NexoraError::invalid_input(format!("invalid encoded track: {error}")))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| NexoraError::invalid_input(format!("invalid encoded track: {error}")))?;
    let identifier = string_field(&value, "track_id").unwrap_or_default();
    let title = string_field(&value, "title").unwrap_or_default();
    let duration = value.get("duration_ms").and_then(Value::as_u64);
    Ok(V4TrackInfo {
        is_seekable: duration.is_some(),
        author: string_field(&value, "artist").unwrap_or_else(|| "unknown".to_owned()),
        length: duration.unwrap_or(0),
        is_stream: false,
        position: 0,
        uri: value.get("uri").and_then(Value::as_str).map(str::to_owned),
        artwork_url: value
            .get("artwork_url")
            .and_then(Value::as_str)
            .map(str::to_owned),
        isrc: value.get("isrc").and_then(Value::as_str).map(str::to_owned),
        source_name: string_field(&value, "source_id").unwrap_or_else(|| "local".to_owned()),
        title,
        identifier,
    })
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Fold the caller's source payload into the encoded payload without
/// letting it overwrite the identity fields.
fn merge_source_payload(mut payload: Value, source_payload: &Value) -> Value {
    if let (Some(target), Some(source)) = (payload.as_object_mut(), source_payload.as_object()) {
        for (key, value) in source {
            if !matches!(
                key.as_str(),
                "track_id" | "source_id" | "title" | "duration_ms"
            ) {
                target.insert(key.clone(), value.clone());
            }
        }
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde_json::json;

    fn sample_info() -> V4TrackInfo {
        V4TrackInfo {
            identifier: "tone.wav".into(),
            is_seekable: true,
            author: "artist".into(),
            length: 100,
            is_stream: false,
            position: 0,
            title: "Tone".into(),
            uri: None,
            artwork_url: None,
            isrc: None,
            source_name: "local".into(),
        }
    }

    #[test]
    fn encode_decode_round_trip_is_lossless() {
        let info = sample_info();
        let encoded = encode_track(&info, &json!({"source_id": "local"})).unwrap();
        let decoded = decode_track(&encoded).unwrap();
        assert_eq!(decoded.identifier, "tone.wav");
        assert_eq!(decoded.title, "Tone");
        assert_eq!(decoded.length, 100);
        assert_eq!(decoded.author, "artist");
        assert_eq!(decoded.source_name, "local");
        assert!(decoded.is_seekable);
    }

    #[test]
    fn decode_track_defaults_missing_fields() {
        let payload = json!({"track_id": "x", "source_id": "local", "title": "T"}).to_string();
        let encoded = STANDARD.encode(payload);
        let decoded = decode_track(&encoded).unwrap();
        assert_eq!(decoded.length, 0);
        assert_eq!(decoded.author, "unknown");
        assert!(!decoded.is_seekable);
    }

    #[test]
    fn decode_track_rejects_malformed_input() {
        assert!(decode_track("not base64!!").is_err());
        assert!(decode_track(&STANDARD.encode([0xff, 0xfe])).is_err());
        assert!(decode_track(&STANDARD.encode("not json")).is_err());
    }
}
