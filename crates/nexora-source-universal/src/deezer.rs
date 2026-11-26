use std::sync::Arc;

use async_trait::async_trait;
use nexora_common::TrackId;
use nexora_source_api::{
    AudioSource, AuthContext, ResolvedSource, SourceCapabilities, SourceError, SourceHealth,
    TrackMetadata, TrackReference,
};
use serde_json::Value;

use crate::oauth::TokenStore;

const SOURCE_ID: &str = "deezer";

pub fn require_token(store: &TokenStore) -> Result<String, SourceError> {
    store.get(SOURCE_ID).ok_or(SourceError::Unauthorized)
}

pub fn safe_deezer_url(url: &str) -> Result<&str, SourceError> {
    let host = crate::http::host_of(url)?;
    if host == "deezer.com"
        || host == "www.deezer.com"
        || host == "api.deezer.com"
        || host == "cdnt-preview.deezer.com"
    {
        Ok(url)
    } else {
        Err(SourceError::InvalidInput)
    }
}

fn identifier(value: &Value) -> Result<String, SourceError> {
    value
        .get("id")
        .and_then(|id| {
            id.as_str()
                .map(str::to_owned)
                .or_else(|| id.as_u64().map(|n| n.to_string()))
        })
        .ok_or(SourceError::InvalidInput)
}

fn artist_name(item: &Value) -> Option<String> {
    item.get("artist").and_then(|artist| {
        artist
            .as_str()
            .map(str::to_owned)
            .or_else(|| artist.get("name")?.as_str().map(str::to_owned))
    })
}

pub fn parse_search_response(body: &str) -> Result<Vec<TrackMetadata>, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let items = root
        .get("data")
        .and_then(Value::as_array)
        .ok_or(SourceError::Unavailable)?;
    items
        .iter()
        .map(|item| {
            let id = identifier(item)?;
            let duration_ms = item
                .get("duration")
                .and_then(Value::as_u64)
                .filter(|seconds| *seconds > 0)
                .map(|seconds| seconds * 1_000);
            Ok(TrackMetadata {
                track_id: TrackId::new(id).map_err(|_| SourceError::InvalidInput)?,
                title: item
                    .get("title")
                    .and_then(Value::as_str)
                    .ok_or(SourceError::InvalidInput)?
                    .to_owned(),
                artist: artist_name(item),
                duration_ms,
                source_id: SOURCE_ID.to_owned(),
                available: item.get("preview").and_then(Value::as_str).is_some(),
            })
        })
        .collect()
}

pub fn resolve_details(body: &str) -> Result<ResolvedSource, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let url = root
        .get("preview")
        .and_then(Value::as_str)
        .ok_or(SourceError::NotFound)?;
    safe_deezer_url(url)?;
    Ok(ResolvedSource::Stream {
        url: url.to_owned(),
    })
}

pub struct DeezerSource {
    client: reqwest::Client,
    tokens: Arc<TokenStore>,
}

impl DeezerSource {
    pub fn new(tokens: Arc<TokenStore>) -> Self {
        Self {
            client: reqwest::Client::new(),
            tokens,
        }
    }

    fn search_url(query: &str) -> Result<reqwest::Url, SourceError> {
        let mut url = reqwest::Url::parse("https://api.deezer.com/search")
            .map_err(|_| SourceError::Unavailable)?;
        url.query_pairs_mut().append_pair("q", query);
        Ok(url)
    }

    fn details_url(track: &TrackReference) -> Result<reqwest::Url, SourceError> {
        let raw = if track.query.starts_with("https://") {
            safe_deezer_url(&track.query)?
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .ok_or(SourceError::InvalidInput)?
        } else {
            track.query.as_str()
        };
        if raw.is_empty() || raw.len() > 128 {
            return Err(SourceError::InvalidInput);
        }
        reqwest::Url::parse(&format!("https://api.deezer.com/track/{raw}"))
            .map_err(|_| SourceError::InvalidInput)
    }

    async fn get_text(&self, url: reqwest::Url) -> Result<String, SourceError> {
        require_token(&self.tokens)?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| SourceError::Unavailable)?;
        match response.status().as_u16() {
            200 => response.text().await.map_err(|_| SourceError::Unavailable),
            401 | 403 => Err(SourceError::Unauthorized),
            429 => Err(SourceError::RateLimited),
            _ => Err(SourceError::Unavailable),
        }
    }
}

#[async_trait]
impl AudioSource for DeezerSource {
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
        let body = self.get_text(Self::search_url(query)?).await?;
        parse_search_response(&body)
    }

    async fn resolve(
        &self,
        track: &TrackReference,
        _context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        let body = self.get_text(Self::details_url(track)?).await?;
        resolve_details(&body)
    }

    async fn health(&self, _context: &AuthContext) -> Result<SourceHealth, SourceError> {
        match self.get_text(Self::search_url("health")?).await {
            Ok(_) => Ok(SourceHealth::healthy(SOURCE_ID)),
            Err(error) => Ok(SourceHealth::unhealthy(SOURCE_ID, format!("{error:?}"))),
        }
    }
}
