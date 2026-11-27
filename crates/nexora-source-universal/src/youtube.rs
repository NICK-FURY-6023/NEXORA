use async_trait::async_trait;
use nexora_common::TrackId;
use nexora_source_api::{
    AudioSource, AuthContext, ResolvedSource, SourceCapabilities, SourceError, SourceHealth,
    TrackMetadata, TrackReference,
};
use serde_json::Value;

use crate::http;

const SOURCE_ID: &str = "youtube";
const PLAYER_URL: &str = "https://www.youtube.com/youtubei/v1/player";

pub fn map_status(status: u16) -> SourceError {
    match status {
        401 | 403 => SourceError::Unauthorized,
        429 => SourceError::RateLimited,
        _ => SourceError::Unavailable,
    }
}

/// Accepts only the hosts YouTube serves watch pages and media from. The
/// googlevideo CDN hosts are per-shard, so the suffix check is used there.
pub fn safe_youtube_url(url: &str) -> Result<&str, SourceError> {
    let host = http::host_of(url)?;
    let allowed = host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtu.be"
        || host == "googlevideo.com"
        || host.ends_with(".googlevideo.com");
    if allowed {
        Ok(url)
    } else {
        Err(SourceError::InvalidInput)
    }
}

/// YouTube video ids are 11 base64url characters. The set is kept tight so a
/// value interpolated into a URL cannot escape its path.
fn valid_video_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Extracts a video id from a bare id, a `watch?v=` URL or a `youtu.be` URL.
/// The host is checked first, so a lookalike domain cannot steer the id.
///
/// `Url` normalises `..` segments away, so the path is required to be exactly
/// one segment; a traversal attempt would otherwise read as another video's id.
pub fn video_id(query: &str) -> Result<String, SourceError> {
    let raw = if query.starts_with("https://") {
        safe_youtube_url(query)?;
        let parsed = reqwest::Url::parse(query).map_err(|_| SourceError::InvalidInput)?;
        let from_query = parsed
            .query_pairs()
            .find(|(key, _)| key == "v")
            .map(|(_, value)| value.into_owned());
        match from_query {
            Some(value) => value,
            None => {
                let segments: Vec<&str> = parsed
                    .path_segments()
                    .map(|segments| segments.filter(|s| !s.is_empty()).collect())
                    .unwrap_or_default();
                match segments.as_slice() {
                    [id] => (*id).to_owned(),
                    _ => return Err(SourceError::InvalidInput),
                }
            }
        }
    } else {
        query.to_owned()
    };
    if !valid_video_id(&raw) {
        return Err(SourceError::InvalidInput);
    }
    Ok(raw)
}

/// Builds the Data API search URL. The Data API needs a key; without one the
/// request cannot succeed, so the caller is told the source is unauthorized.
pub fn search_url(query: &str, api_key: Option<&str>) -> Result<reqwest::Url, SourceError> {
    let key = api_key
        .filter(|key| !key.is_empty())
        .ok_or(SourceError::Unauthorized)?;
    let mut url = reqwest::Url::parse("https://www.googleapis.com/youtube/v3/search")
        .map_err(|_| SourceError::Unavailable)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("part", "snippet");
        pairs.append_pair("type", "video");
        pairs.append_pair("q", query);
        pairs.append_pair("maxResults", "20");
        pairs.append_pair("key", key);
    }
    Ok(url)
}

/// Builds the request body for the keyless InnerTube player endpoint, which is
/// the only public source of the `streamingData` object the resolver reads.
pub fn player_body(video_id: &str) -> Result<String, SourceError> {
    if !valid_video_id(video_id) {
        return Err(SourceError::InvalidInput);
    }
    Ok(format!(
        r#"{{"context":{{"client":{{"clientName":"WEB","clientVersion":"2.20240101.00.00"}}}},"videoId":"{video_id}"}}"#
    ))
}

/// Parses a `M:SS` or `H:MM:SS` duration from the YouTube snippet into ms.
fn parse_duration(snippet: Option<&Value>) -> Option<u64> {
    let text = snippet?.get("duration").and_then(Value::as_str)?;
    let mut total_ms = 0u64;
    for part in text.split(':') {
        let seconds = part.parse::<u64>().ok()?;
        total_ms = total_ms
            .checked_mul(60)?
            .checked_add(seconds.checked_mul(1_000)?)?;
    }
    Some(total_ms)
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
                duration_ms: parse_duration(item.pointer("/snippet")),
                source_id: SOURCE_ID.to_owned(),
                available: true,
            })
        })
        .collect()
}

pub fn resolve_details(body: &str) -> Result<ResolvedSource, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let url = root
        .pointer("/streamingData/formats")
        .and_then(Value::as_array)
        .ok_or(SourceError::Unavailable)?
        .iter()
        .filter_map(|format| format.get("url").and_then(Value::as_str))
        .next()
        .ok_or(SourceError::NotFound)?;
    safe_youtube_url(url)?;
    Ok(ResolvedSource::Stream {
        url: url.to_owned(),
    })
}

pub struct YouTubeSource {
    client: reqwest::Client,
    api_key: Option<String>,
}

impl YouTubeSource {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: crate::http::client(),
            api_key,
        }
    }

    async fn get_text(&self, url: reqwest::Url) -> Result<String, SourceError> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| SourceError::Unavailable)?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(map_status(status));
        }
        crate::http::read_text(response).await
    }

    async fn post_json(&self, url: reqwest::Url, body: String) -> Result<String, SourceError> {
        let response = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| SourceError::Unavailable)?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(map_status(status));
        }
        crate::http::read_text(response).await
    }
}

impl Default for YouTubeSource {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl AudioSource for YouTubeSource {
    fn source_id(&self) -> &'static str {
        SOURCE_ID
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::full()
    }

    async fn search(
        &self,
        query: &str,
        _context: &AuthContext,
    ) -> Result<Vec<TrackMetadata>, SourceError> {
        let url = search_url(query, self.api_key.as_deref())?;
        let body = self.get_text(url).await?;
        parse_search_response(&body)
    }

    async fn resolve(
        &self,
        track: &TrackReference,
        _context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        let id = video_id(&track.query)?;
        let url = reqwest::Url::parse(PLAYER_URL).map_err(|_| SourceError::Unavailable)?;
        let body = self.post_json(url, player_body(&id)?).await?;
        resolve_details(&body)
    }

    async fn health(&self, _context: &AuthContext) -> Result<SourceHealth, SourceError> {
        match search_url("health", self.api_key.as_deref()) {
            Err(error) => Ok(SourceHealth::unhealthy(SOURCE_ID, format!("{error:?}"))),
            Ok(url) => match self.get_text(url).await {
                Ok(_) => Ok(SourceHealth::healthy(SOURCE_ID)),
                Err(error) => Ok(SourceHealth::unhealthy(SOURCE_ID, format!("{error:?}"))),
            },
        }
    }
}
