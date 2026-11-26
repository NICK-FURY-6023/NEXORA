use std::sync::Arc;

use async_trait::async_trait;
use nexora_common::TrackId;
use nexora_source_api::{
    AudioSource, AuthContext, ResolvedSource, SourceCapabilities, SourceError, SourceHealth,
    TrackMetadata, TrackReference,
};
use serde_json::Value;

use crate::oauth::TokenStore;
use crate::simple;

const SOURCE_ID: &str = "apple-music";
const STOREFRONT: &str = "us";

pub fn require_token(store: &TokenStore) -> Result<String, SourceError> {
    store.get(SOURCE_ID).ok_or(SourceError::Unauthorized)
}

pub fn safe_apple_url(url: &str) -> Result<&str, SourceError> {
    let rest = url
        .strip_prefix("https://")
        .ok_or(SourceError::InvalidInput)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority
        .rsplit('@')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host == "api.music.apple.com"
        || host == "music.apple.com"
        || host == "audio-ssl.itunes.apple.com"
    {
        Ok(url)
    } else {
        Err(SourceError::InvalidInput)
    }
}

/// Parses the Apple Music catalog search payload.
///
/// The catalog nests songs under `results.songs.data`; anything else falls
/// back to the generic `data` array shape.
pub fn parse_search_response(body: &str) -> Result<Vec<TrackMetadata>, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let Some(items) = root
        .pointer("/results/songs/data")
        .and_then(Value::as_array)
    else {
        return simple::parse(body, SOURCE_ID);
    };

    items
        .iter()
        .map(|item| {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .ok_or(SourceError::InvalidInput)?;
            let attributes = item.get("attributes").ok_or(SourceError::InvalidInput)?;
            let title = attributes
                .get("name")
                .and_then(Value::as_str)
                .ok_or(SourceError::InvalidInput)?;
            Ok(TrackMetadata {
                track_id: TrackId::new(id).map_err(|_| SourceError::InvalidInput)?,
                title: title.to_owned(),
                artist: attributes
                    .get("artistName")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                duration_ms: attributes
                    .get("durationInMillis")
                    .and_then(Value::as_u64)
                    .filter(|ms| *ms > 0),
                source_id: SOURCE_ID.to_owned(),
                available: attributes
                    .get("previewUrl")
                    .and_then(Value::as_str)
                    .is_some(),
            })
        })
        .collect()
}

pub fn resolve_details(body: &str) -> Result<ResolvedSource, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let url = root
        .pointer("/data/0/attributes/previewUrl")
        .and_then(Value::as_str)
        .ok_or(SourceError::NotFound)?;
    safe_apple_url(url)?;
    Ok(ResolvedSource::Stream {
        url: url.to_owned(),
    })
}

pub struct AppleMusicSource {
    client: reqwest::Client,
    tokens: Arc<TokenStore>,
}

impl AppleMusicSource {
    pub fn new(tokens: Arc<TokenStore>) -> Self {
        Self {
            client: crate::http::client(),
            tokens,
        }
    }

    fn search_url(query: &str) -> Result<reqwest::Url, SourceError> {
        let mut url = reqwest::Url::parse(&format!(
            "https://api.music.apple.com/v1/catalog/{STOREFRONT}/search"
        ))
        .map_err(|_| SourceError::Unavailable)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("term", query);
            pairs.append_pair("types", "songs");
            pairs.append_pair("limit", "20");
        }
        Ok(url)
    }

    fn details_url(track: &TrackReference) -> Result<reqwest::Url, SourceError> {
        let raw = if track.query.starts_with("https://") {
            safe_apple_url(&track.query)?
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .ok_or(SourceError::InvalidInput)?
        } else {
            track.query.as_str()
        };
        if raw.is_empty() || raw.len() > 128 || !raw.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(SourceError::InvalidInput);
        }
        reqwest::Url::parse(&format!(
            "https://api.music.apple.com/v1/catalog/{STOREFRONT}/songs/{raw}"
        ))
        .map_err(|_| SourceError::InvalidInput)
    }

    async fn get_text(&self, url: reqwest::Url) -> Result<String, SourceError> {
        let token = require_token(&self.tokens)?;
        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| SourceError::Unavailable)?;
        match response.status().as_u16() {
            200 => crate::http::read_text(response).await,
            401 | 403 => Err(SourceError::Unauthorized),
            429 => Err(SourceError::RateLimited),
            _ => Err(SourceError::Unavailable),
        }
    }
}

#[async_trait]
impl AudioSource for AppleMusicSource {
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
