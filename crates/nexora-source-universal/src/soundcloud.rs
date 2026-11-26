use std::sync::Arc;

use async_trait::async_trait;
use nexora_common::TrackId;
use nexora_source_api::{
    AudioSource, AuthContext, ResolvedSource, SourceCapabilities, SourceError, SourceHealth,
    TrackMetadata, TrackReference,
};
use serde_json::Value;

use crate::oauth::TokenStore;
use crate::oembed;

const SOURCE_ID: &str = "soundcloud-public";
const TOKEN_KEY: &str = "soundcloud";

pub fn parse_oembed(body: &str) -> Result<TrackMetadata, SourceError> {
    oembed::parse(body, SOURCE_ID)
}

/// SoundCloud's public API needs a registered client id, not a user login.
pub fn require_client_id(store: &TokenStore) -> Result<String, SourceError> {
    store.get(TOKEN_KEY).ok_or(SourceError::Unauthorized)
}

pub fn safe_soundcloud_url(url: &str) -> Result<&str, SourceError> {
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
    if host == "soundcloud.com" || host.ends_with(".soundcloud.com") {
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
                .or_else(|| id.as_u64().map(|number| number.to_string()))
        })
        .ok_or(SourceError::InvalidInput)
}

fn transcoding_url(item: &Value) -> Option<&str> {
    let transcodings = item.pointer("/media/transcodings")?.as_array()?;
    let progressive = transcodings.iter().find(|entry| {
        entry
            .pointer("/format/protocol")
            .and_then(Value::as_str)
            .is_some_and(|protocol| protocol == "progressive")
    });
    progressive
        .or_else(|| transcodings.first())
        .and_then(|entry| entry.get("url"))
        .and_then(Value::as_str)
}

pub fn parse_search_response(body: &str) -> Result<Vec<TrackMetadata>, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let items = root
        .get("collection")
        .and_then(Value::as_array)
        .ok_or(SourceError::Unavailable)?;

    items
        .iter()
        .map(|item| {
            let id = identifier(item)?;
            let duration_ms = item
                .get("duration")
                .and_then(Value::as_u64)
                .filter(|ms| *ms > 0);
            Ok(TrackMetadata {
                track_id: TrackId::new(id).map_err(|_| SourceError::InvalidInput)?,
                title: item
                    .get("title")
                    .and_then(Value::as_str)
                    .ok_or(SourceError::InvalidInput)?
                    .to_owned(),
                artist: item
                    .pointer("/user/username")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                duration_ms,
                source_id: SOURCE_ID.to_owned(),
                available: transcoding_url(item).is_some(),
            })
        })
        .collect()
}

pub fn resolve_details(body: &str, client_id: &str) -> Result<ResolvedSource, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let url = transcoding_url(&root).ok_or(SourceError::NotFound)?;
    safe_soundcloud_url(url)?;
    let mut parsed = reqwest::Url::parse(url).map_err(|_| SourceError::Unavailable)?;
    let kept: Vec<(String, String)> = parsed
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .filter(|(key, _)| key != "client_id")
        .collect();
    {
        let mut pairs = parsed.query_pairs_mut();
        pairs.clear();
        for (key, value) in kept {
            pairs.append_pair(&key, &value);
        }
        pairs.append_pair("client_id", client_id);
    }
    Ok(ResolvedSource::Stream {
        url: parsed.to_string(),
    })
}

pub struct SoundcloudSource {
    client: reqwest::Client,
    tokens: Arc<TokenStore>,
}

impl SoundcloudSource {
    pub fn new(tokens: Arc<TokenStore>) -> Self {
        Self {
            client: reqwest::Client::new(),
            tokens,
        }
    }

    fn search_url(query: &str, client_id: &str) -> Result<reqwest::Url, SourceError> {
        let mut url = reqwest::Url::parse("https://api-v2.soundcloud.com/search/tracks")
            .map_err(|_| SourceError::Unavailable)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", query);
            pairs.append_pair("client_id", client_id);
            pairs.append_pair("limit", "20");
        }
        Ok(url)
    }

    fn details_url(track: &TrackReference, client_id: &str) -> Result<reqwest::Url, SourceError> {
        let raw = if track.query.starts_with("https://") {
            safe_soundcloud_url(&track.query)?
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
        let mut url = reqwest::Url::parse(&format!("https://api-v2.soundcloud.com/tracks/{raw}"))
            .map_err(|_| SourceError::InvalidInput)?;
        url.query_pairs_mut().append_pair("client_id", client_id);
        Ok(url)
    }

    async fn get_text(&self, url: reqwest::Url) -> Result<String, SourceError> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| SourceError::Unavailable)?;
        match response.status().as_u16() {
            200 => response.text().await.map_err(|_| SourceError::Unavailable),
            401 | 403 => Err(SourceError::Unauthorized),
            404 => Err(SourceError::NotFound),
            429 => Err(SourceError::RateLimited),
            _ => Err(SourceError::Unavailable),
        }
    }
}

#[async_trait]
impl AudioSource for SoundcloudSource {
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
        let client_id = require_client_id(&self.tokens)?;
        let body = self.get_text(Self::search_url(query, &client_id)?).await?;
        parse_search_response(&body)
    }

    async fn resolve(
        &self,
        track: &TrackReference,
        _context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        let client_id = require_client_id(&self.tokens)?;
        let body = self.get_text(Self::details_url(track, &client_id)?).await?;
        resolve_details(&body, &client_id)
    }

    async fn health(&self, _context: &AuthContext) -> Result<SourceHealth, SourceError> {
        let client_id = match require_client_id(&self.tokens) {
            Ok(client_id) => client_id,
            Err(error) => {
                return Ok(SourceHealth::unhealthy(SOURCE_ID, format!("{error:?}")));
            }
        };
        match self.get_text(Self::search_url("health", &client_id)?).await {
            Ok(_) => Ok(SourceHealth::healthy(SOURCE_ID)),
            Err(error) => Ok(SourceHealth::unhealthy(SOURCE_ID, format!("{error:?}"))),
        }
    }
}
