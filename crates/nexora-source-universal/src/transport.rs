use async_trait::async_trait;
use nexora_source_api::{
    AudioSource, AuthContext, ResolvedSource, SourceCapabilities, SourceError, SourceHealth,
    TrackMetadata, TrackReference,
};

use crate::archive;

pub struct InternetArchiveSource {
    client: reqwest::Client,
}

impl InternetArchiveSource {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn search_url(query: &str) -> Result<reqwest::Url, SourceError> {
        let mut url = reqwest::Url::parse("https://archive.org/advancedsearch.php")
            .map_err(|_| SourceError::Unavailable)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", query);
            pairs.append_pair("fl[]", "identifier");
            pairs.append_pair("fl[]", "title");
            pairs.append_pair("fl[]", "creator");
            pairs.append_pair("fl[]", "length");
            pairs.append_pair("rows", "20");
            pairs.append_pair("output", "json");
        }
        Ok(url)
    }

    fn details_url(identifier: &str) -> Result<reqwest::Url, SourceError> {
        if identifier.is_empty() || identifier.len() > 128 {
            return Err(SourceError::InvalidInput);
        }
        let url = format!("https://archive.org/metadata/{identifier}");
        reqwest::Url::parse(&url).map_err(|_| SourceError::InvalidInput)
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
            return Err(if status == 429 {
                SourceError::RateLimited
            } else {
                SourceError::Unavailable
            });
        }
        response.text().await.map_err(|_| SourceError::Unavailable)
    }
}

impl Default for InternetArchiveSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioSource for InternetArchiveSource {
    fn source_id(&self) -> &'static str {
        "internet-archive"
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::full()
    }

    async fn search(
        &self,
        query: &str,
        _context: &AuthContext,
    ) -> Result<Vec<TrackMetadata>, SourceError> {
        let url = Self::search_url(query)?;
        let body = self.get_text(url).await?;
        archive::parse_search_response(&body)
    }

    async fn resolve(
        &self,
        track: &TrackReference,
        _context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        let url = Self::details_url(&track.query)?;
        let body = self.get_text(url).await?;
        archive::resolve_details(&body)
    }

    async fn health(&self, _context: &AuthContext) -> Result<SourceHealth, SourceError> {
        let url = Self::search_url("health")?;
        match self.get_text(url).await {
            Ok(_) => Ok(SourceHealth::healthy("internet-archive")),
            Err(error) => Ok(SourceHealth::unhealthy(
                "internet-archive",
                format!("{error:?}"),
            )),
        }
    }
}
