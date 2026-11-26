use async_trait::async_trait;
use nexora_common::TrackId;
use nexora_source_api::{
    AudioSource, AuthContext, ResolvedSource, SourceCapabilities, SourceError, SourceHealth,
    TrackMetadata, TrackReference,
};
use serde_json::Value;

use crate::oembed;

const SOURCE_ID: &str = "bandcamp-public";

/// The tralbum identity Bandcamp embeds in its player markup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TralbumIds {
    pub band_id: u64,
    pub tralbum_id: u64,
    pub tralbum_type: String,
}

pub fn parse_oembed(body: &str) -> Result<TrackMetadata, SourceError> {
    oembed::parse(body, SOURCE_ID)
}

pub fn safe_bandcamp_url(url: &str) -> Result<&str, SourceError> {
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
    if host == "bandcamp.com"
        || host == "www.bandcamp.com"
        || host.ends_with(".bandcamp.com")
        || host == "bcbits.com"
        || host.ends_with(".bcbits.com")
    {
        Ok(url)
    } else {
        Err(SourceError::InvalidInput)
    }
}

/// Parses the Bandcamp public search payload.
pub fn parse_search_response(body: &str) -> Result<Vec<TrackMetadata>, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let rows = root
        .pointer("/auto/results")
        .or_else(|| root.get("results"))
        .and_then(Value::as_array)
        .ok_or(SourceError::Unavailable)?;

    rows.iter()
        .map(|row| {
            let item = row.get("auto").unwrap_or(row);
            let id = item
                .get("item_id")
                .and_then(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| value.as_u64().map(|number| number.to_string()))
                })
                .ok_or(SourceError::InvalidInput)?;
            let title = item
                .get("item_title")
                .and_then(Value::as_str)
                .ok_or(SourceError::InvalidInput)?;
            Ok(TrackMetadata {
                track_id: TrackId::new(id).map_err(|_| SourceError::InvalidInput)?,
                title: title.to_owned(),
                artist: item
                    .get("band_name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                duration_ms: None,
                source_id: SOURCE_ID.to_owned(),
                available: true,
            })
        })
        .collect()
}

/// Extracts the tralbum identity from an oEmbed payload's player markup.
pub fn parse_tralbum_ids(body: &str) -> Result<TralbumIds, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let html = root
        .get("html")
        .and_then(Value::as_str)
        .ok_or(SourceError::InvalidInput)?;
    let marker = "data-tralbum=";
    let start = html.find(marker).ok_or(SourceError::InvalidInput)? + marker.len();
    let raw = html[start..].trim_start_matches(['\'', '"']);
    let tralbum = first_json_value(raw)
        .or_else(|| first_json_value(&decode_entities(raw)))
        .ok_or(SourceError::InvalidInput)?;

    let band_id = tralbum
        .get("band_id")
        .and_then(Value::as_u64)
        .ok_or(SourceError::InvalidInput)?;
    let tralbum_id = tralbum
        .get("tralbum_id")
        .and_then(Value::as_u64)
        .ok_or(SourceError::InvalidInput)?;
    let tralbum_type = tralbum
        .get("tralbum_type")
        .and_then(Value::as_str)
        .ok_or(SourceError::InvalidInput)?
        .to_owned();
    Ok(TralbumIds {
        band_id,
        tralbum_id,
        tralbum_type,
    })
}

/// Reads the first JSON value in the text, ignoring whatever follows it.
fn first_json_value(text: &str) -> Option<Value> {
    serde_json::Deserializer::from_str(text)
        .into_iter::<Value>()
        .next()
        .and_then(Result::ok)
}

/// The embed carries its JSON HTML-escaped; the entity names are split so
/// they cannot be mistaken for markup.
fn decode_entities(value: &str) -> String {
    value
        .replace(concat!("&", "quot;"), "\"")
        .replace(concat!("&", "#39;"), "'")
        .replace(concat!("&", "lt;"), "<")
        .replace(concat!("&", "gt;"), ">")
        .replace(concat!("&", "amp;"), "&")
}

/// Resolves the tralbum details payload to its mp3 stream.
pub fn resolve_details(body: &str) -> Result<ResolvedSource, SourceError> {
    let root: Value = serde_json::from_str(body).map_err(|_| SourceError::Unavailable)?;
    let url = root
        .pointer("/streaminfo/mp3-128/url")
        .and_then(Value::as_str)
        .ok_or(SourceError::NotFound)?;
    safe_bandcamp_url(url)?;
    Ok(ResolvedSource::Stream {
        url: url.to_owned(),
    })
}

pub struct BandcampSource {
    client: reqwest::Client,
}

impl BandcampSource {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn oembed_url(track_url: &str) -> Result<reqwest::Url, SourceError> {
        let track_url = safe_bandcamp_url(track_url)?;
        let mut url = reqwest::Url::parse("https://bandcamp.com/api/oembed")
            .map_err(|_| SourceError::Unavailable)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("url", track_url);
            pairs.append_pair("format", "json");
        }
        Ok(url)
    }

    fn details_url(ids: &TralbumIds) -> Result<reqwest::Url, SourceError> {
        let mut url = reqwest::Url::parse("https://bandcamp.com/api/mobile/24/tralbum_details")
            .map_err(|_| SourceError::Unavailable)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("band_id", &ids.band_id.to_string());
            pairs.append_pair("tralbum_id", &ids.tralbum_id.to_string());
            pairs.append_pair("tralbum_type", &ids.tralbum_type);
        }
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
            404 => Err(SourceError::NotFound),
            429 => Err(SourceError::RateLimited),
            _ => Err(SourceError::Unavailable),
        }
    }

    async fn search_body(&self, query: &str) -> Result<String, SourceError> {
        let url = reqwest::Url::parse(
            "https://bandcamp.com/api/bcsearch_public_api/1/autocomplete_elastic",
        )
        .map_err(|_| SourceError::Unavailable)?;
        let payload = format!(
            "{{\"search_text\":{},\"search_filter\":\"t\",\"full_page\":true,\"fan_id\":null}}",
            serde_json::to_string(query).map_err(|_| SourceError::InvalidInput)?
        );
        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await
            .map_err(|_| SourceError::Unavailable)?;
        match response.status().as_u16() {
            200 => response.text().await.map_err(|_| SourceError::Unavailable),
            429 => Err(SourceError::RateLimited),
            _ => Err(SourceError::Unavailable),
        }
    }
}

impl Default for BandcampSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioSource for BandcampSource {
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
        let body = self.search_body(query).await?;
        parse_search_response(&body)
    }

    async fn resolve(
        &self,
        track: &TrackReference,
        _context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        let oembed = self.get_text(Self::oembed_url(&track.query)?).await?;
        let ids = parse_tralbum_ids(&oembed)?;
        let body = self.get_text(Self::details_url(&ids)?).await?;
        resolve_details(&body)
    }

    async fn health(&self, _context: &AuthContext) -> Result<SourceHealth, SourceError> {
        let oembed = self
            .get_text(Self::oembed_url("https://bandcamp.com/track/health")?)
            .await;
        match oembed {
            Ok(_) => Ok(SourceHealth::healthy(SOURCE_ID)),
            Err(error) => Ok(SourceHealth::unhealthy(SOURCE_ID, format!("{error:?}"))),
        }
    }
}
