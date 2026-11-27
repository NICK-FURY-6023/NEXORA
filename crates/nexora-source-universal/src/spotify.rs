use std::sync::Arc;

use async_trait::async_trait;
use nexora_common::TrackId;
use nexora_source_api::{
    AudioSource, AuthContext, ResolvedSource, SourceCapabilities, SourceError, SourceHealth,
    TrackMetadata, TrackReference,
};
use serde_json::Value;

use crate::{http, oauth::TokenStore};

const SOURCE_ID: &str = "spotify";

pub fn require_token(store: &TokenStore) -> Result<String, SourceError> {
    store.get(SOURCE_ID).ok_or(SourceError::Unauthorized)
}

/// Accepts only the hosts Spotify itself serves from: the public site, the
/// web API and the preview CDN. Anything else is refused before a request.
pub fn safe_spotify_url(url: &str) -> Result<&str, SourceError> {
    let host = http::host_of(url)?;
    let allowed = host == "spotify.com"
        || host.ends_with(".spotify.com")
        || host == "scdn.co"
        || host.ends_with(".scdn.co");
    if allowed {
        Ok(url)
    } else {
        Err(SourceError::InvalidInput)
    }
}

/// Spotify track ids are base62; the extra characters cover the ids the web
/// player exposes. Keeping the set tight stops path traversal in a URL we
/// build from user input.
fn valid_track_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Extracts a track id from a bare id or from an `open.spotify.com/track/...`
/// URL. The host is checked first, so a lookalike domain cannot steer the id.
///
/// The `track` path segment is required: `Url` normalises `..` segments away,
/// so a traversal attempt would otherwise be read as some other track's id.
pub fn track_id(query: &str) -> Result<String, SourceError> {
    let raw = if query.starts_with("https://") {
        safe_spotify_url(query)?;
        let parsed = reqwest::Url::parse(query).map_err(|_| SourceError::InvalidInput)?;
        let segments: Vec<&str> = parsed
            .path_segments()
            .map(|segments| segments.filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        match segments.as_slice() {
            [.., "track", id] => (*id).to_owned(),
            _ => return Err(SourceError::InvalidInput),
        }
    } else {
        query.to_owned()
    };
    if !valid_track_id(&raw) {
        return Err(SourceError::InvalidInput);
    }
    Ok(raw)
}

pub fn parse_search_response(body: &str) -> Result<Vec<TrackMetadata>, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let items = root
        .pointer("/tracks/items")
        .or_else(|| root.get("items"))
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
                source_id: SOURCE_ID.to_owned(),
                available: item.get("preview_url").and_then(Value::as_str).is_some(),
            })
        })
        .collect()
}

pub fn resolve_details(body: &str) -> Result<ResolvedSource, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let url = root
        .get("preview_url")
        .and_then(Value::as_str)
        .or_else(|| {
            root.get("items")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("preview_url"))
                .and_then(Value::as_str)
        })
        .ok_or(SourceError::NotFound)?;
    safe_spotify_url(url)?;
    Ok(ResolvedSource::Stream {
        url: url.to_owned(),
    })
}

pub struct SpotifySource {
    client: reqwest::Client,
    tokens: Arc<TokenStore>,
}

impl SpotifySource {
    pub fn new(tokens: Arc<TokenStore>) -> Self {
        Self {
            client: crate::http::client(),
            tokens,
        }
    }

    fn search_url(query: &str) -> Result<reqwest::Url, SourceError> {
        let mut url = reqwest::Url::parse("https://api.spotify.com/v1/search")
            .map_err(|_| SourceError::Unavailable)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", query);
            pairs.append_pair("type", "track");
            pairs.append_pair("limit", "20");
        }
        Ok(url)
    }

    fn details_url(track: &TrackReference) -> Result<reqwest::Url, SourceError> {
        let raw = track_id(&track.query)?;
        reqwest::Url::parse(&format!("https://api.spotify.com/v1/tracks/{raw}"))
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
impl AudioSource for SpotifySource {
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
