use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};

use nexora_common::TrackId;
use nexora_source_api::{
    AudioSource, AuthContext, ResolvedSource, SourceError, SourceHealth, SourceRegistryEntry,
    TrackMetadata, TrackReference,
};
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub struct CancellationToken {
    receiver: watch::Receiver<bool>,
}

#[derive(Debug, Clone)]
pub struct CancellationHandle {
    sender: watch::Sender<bool>,
}

impl CancellationToken {
    pub fn new() -> (CancellationHandle, Self) {
        let (sender, receiver) = watch::channel(false);
        (CancellationHandle { sender }, Self { receiver })
    }

    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    pub async fn wait(&mut self) {
        while !*self.receiver.borrow_and_update() {
            if self.receiver.changed().await.is_err() {
                break;
            }
        }
    }
}

impl CancellationHandle {
    pub fn cancel(&self) {
        let _ = self.sender.send(true);
    }
}

#[derive(Default)]
pub struct SourceRegistry {
    sources: RwLock<Vec<Arc<dyn AudioSource>>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, source: Box<dyn AudioSource>) -> Result<(), SourceError> {
        let mut sources = self.sources.write().map_err(|_| SourceError::Internal)?;
        if sources
            .iter()
            .any(|value| value.source_id() == source.source_id())
        {
            return Err(SourceError::InvalidInput);
        }
        sources.push(Arc::from(source));
        Ok(())
    }

    pub fn source_ids(&self) -> Vec<&'static str> {
        self.sources
            .read()
            .map(|sources| sources.iter().map(|value| value.source_id()).collect())
            .unwrap_or_default()
    }

    pub async fn search(
        &self,
        query: &str,
        context: &AuthContext,
    ) -> Result<Vec<TrackMetadata>, SourceError> {
        self.search_with_timeout(query, context, std::time::Duration::from_secs(3))
            .await
    }

    pub async fn search_with_timeout(
        &self,
        query: &str,
        context: &AuthContext,
        timeout: std::time::Duration,
    ) -> Result<Vec<TrackMetadata>, SourceError> {
        let sources: Vec<Arc<dyn AudioSource>> = self
            .sources
            .read()
            .map_err(|_| SourceError::Internal)?
            .clone();
        let mut results = Vec::new();
        for source in sources {
            if !source.capabilities().search {
                continue;
            }
            match tokio::time::timeout(timeout, source.search(query, context)).await {
                Ok(Ok(mut tracks)) => results.append(&mut tracks),
                Ok(Err(SourceError::Unauthorized)) => continue,
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err(SourceError::Timeout),
            }
        }
        Ok(results)
    }

    pub async fn resolve(
        &self,
        track: &TrackReference,
        context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        let sources: Vec<Arc<dyn AudioSource>> = self
            .sources
            .read()
            .map_err(|_| SourceError::Internal)?
            .clone();
        let ordered = rank_sources(sources, track);
        for source in ordered {
            if !source.capabilities().resolve {
                continue;
            }
            match source.resolve(track, context).await {
                Ok(resolved) => return Ok(resolved),
                Err(SourceError::NotFound) | Err(SourceError::Unavailable) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(SourceError::NotFound)
    }

    pub async fn health_all(&self, context: &AuthContext) -> Vec<SourceHealth> {
        let sources: Vec<Arc<dyn AudioSource>> = match self.sources.read() {
            Ok(sources) => sources.clone(),
            Err(_) => return Vec::new(),
        };
        let mut reports = Vec::with_capacity(sources.len());
        for source in sources {
            let report = match source.health(context).await {
                Ok(report) => report,
                Err(error) => SourceHealth::unhealthy(source.source_id(), error.to_string()),
            };
            reports.push(report);
        }
        reports
    }

    pub fn entries(&self) -> Vec<SourceRegistryEntry> {
        self.sources
            .read()
            .map(|sources| {
                sources
                    .iter()
                    .map(|source| SourceRegistryEntry {
                        source_id: source.source_id(),
                        capabilities: source.capabilities(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn rank_sources(
    sources: Vec<Arc<dyn AudioSource>>,
    track: &TrackReference,
) -> Vec<Arc<dyn AudioSource>> {
    let mut ranked = sources;
    if let Some(hint) = track.source_hint.as_deref() {
        ranked.sort_by_key(|source| if source.source_id() == hint { 0 } else { 1 });
    }
    ranked
}

/// Rank candidates by query relevance, source preference and availability.
pub fn rank_candidates(
    candidates: Vec<TrackMetadata>,
    query: &str,
    preferred_source: Option<&str>,
) -> Vec<TrackMetadata> {
    let needle = query.to_ascii_lowercase();
    let mut ranked = candidates;
    ranked.sort_by_key(|track| {
        let title = track.title.to_ascii_lowercase();
        let relevance = if title == needle {
            0
        } else if title.contains(&needle) {
            1
        } else {
            2
        };
        let source_rank = match preferred_source {
            Some(source) if track.source_id == source => 0,
            Some(_) => 1,
            None => 0,
        };
        let availability = if track.available { 0 } else { 1 };
        (relevance, source_rank, availability)
    });
    ranked
}

/// Validate that a URL is an http(s) URL without an obvious private-network target.
///
/// DNS resolution is intentionally not performed here; egress policy and
/// DNS-rebinding protection belong to the deployment layer.
pub fn validate_public_http_url(url: &str) -> Result<(), SourceError> {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .ok_or(SourceError::InvalidInput)?;
    let authority = rest.split('/').next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    let host = if host_port.starts_with('[') {
        host_port
            .split(']')
            .next()
            .unwrap_or_default()
            .trim_start_matches('[')
    } else {
        host_port.split(':').next().unwrap_or_default()
    };
    if host.is_empty()
        || host.eq_ignore_ascii_case("localhost")
        || host.to_ascii_lowercase().ends_with(".localhost")
    {
        return Err(SourceError::InvalidInput);
    }
    if let Ok(address) = host.parse::<std::net::IpAddr>() {
        let blocked = match address {
            std::net::IpAddr::V4(value) => {
                value.is_loopback()
                    || value.is_private()
                    || value.is_link_local()
                    || value.is_unspecified()
                    || value.is_multicast()
            }
            std::net::IpAddr::V6(value) => {
                value.is_loopback() || value.is_unspecified() || value.is_multicast()
            }
        };
        if blocked {
            return Err(SourceError::InvalidInput);
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct LocalMediaAdapter {
    root: PathBuf,
}

impl LocalMediaAdapter {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, SourceError> {
        let root = root.into();
        if !root.is_dir() {
            return Err(SourceError::InvalidInput);
        }
        Ok(Self { root })
    }
}

#[async_trait::async_trait]
impl AudioSource for LocalMediaAdapter {
    fn source_id(&self) -> &'static str {
        "local-media"
    }

    async fn search(
        &self,
        query: &str,
        context: &AuthContext,
    ) -> Result<Vec<TrackMetadata>, SourceError> {
        if !context.allows("media.local") {
            return Err(SourceError::Unauthorized);
        }
        let mut tracks = Vec::new();
        let entries = std::fs::read_dir(&self.root).map_err(|_| SourceError::Unavailable)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if !name
                .to_ascii_lowercase()
                .contains(&query.to_ascii_lowercase())
            {
                continue;
            }
            if !matches!(
                path.extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .as_str(),
                "wav" | "flac" | "mp3" | "ogg" | "opus" | "aac" | "m4a"
            ) {
                continue;
            }
            let track_id = TrackId::new(name).map_err(|_| SourceError::InvalidInput)?;
            tracks.push(TrackMetadata::local(
                track_id.as_str(),
                name.to_string(),
                path,
            ));
        }
        Ok(tracks)
    }

    async fn resolve(
        &self,
        track: &TrackReference,
        context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        if !context.allows("media.local") {
            return Err(SourceError::Unauthorized);
        }
        let candidate = Path::new(&track.query);
        if candidate.is_absolute()
            || candidate.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(SourceError::InvalidInput);
        }
        let path = self.root.join(candidate);
        if !path.is_file() {
            return Err(SourceError::NotFound);
        }
        Ok(ResolvedSource::LocalFile { path })
    }

    async fn health(&self, context: &AuthContext) -> Result<SourceHealth, SourceError> {
        if !context.allows("media.local") {
            return Err(SourceError::Unauthorized);
        }
        if self.root.is_dir() {
            Ok(SourceHealth::healthy(self.source_id()))
        } else {
            Ok(SourceHealth::unhealthy(self.source_id(), "root missing"))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioStation {
    pub id: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct PublicRadioAdapter {
    stations: Vec<RadioStation>,
}

impl PublicRadioAdapter {
    pub fn new(stations: Vec<RadioStation>) -> Result<Self, SourceError> {
        for station in &stations {
            validate_public_http_url(&station.url)?;
            TrackId::new(&station.id).map_err(|_| SourceError::InvalidInput)?;
        }
        Ok(Self { stations })
    }
}

#[async_trait::async_trait]
impl AudioSource for PublicRadioAdapter {
    fn source_id(&self) -> &'static str {
        "public-radio"
    }

    async fn search(
        &self,
        query: &str,
        context: &AuthContext,
    ) -> Result<Vec<TrackMetadata>, SourceError> {
        if !context.allows("radio.public") {
            return Err(SourceError::Unauthorized);
        }
        let needle = query.to_ascii_lowercase();
        let mut tracks = Vec::new();
        for station in &self.stations {
            if !station.title.to_ascii_lowercase().contains(&needle) {
                continue;
            }
            tracks.push(TrackMetadata {
                track_id: TrackId::new(&station.id).map_err(|_| SourceError::InvalidInput)?,
                title: station.title.clone(),
                artist: None,
                duration_ms: None,
                source_id: self.source_id().to_string(),
                available: true,
            });
        }
        Ok(tracks)
    }

    async fn resolve(
        &self,
        track: &TrackReference,
        context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        if !context.allows("radio.public") {
            return Err(SourceError::Unauthorized);
        }
        let station = self
            .stations
            .iter()
            .find(|value| value.id == track.query || value.title.eq_ignore_ascii_case(&track.query))
            .ok_or(SourceError::NotFound)?;
        validate_public_http_url(&station.url)?;
        Ok(ResolvedSource::Stream {
            url: station.url.clone(),
        })
    }

    async fn health(&self, _context: &AuthContext) -> Result<SourceHealth, SourceError> {
        Ok(SourceHealth::healthy(self.source_id()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct AuthorizedHttpAdapter;

#[async_trait::async_trait]
impl AudioSource for AuthorizedHttpAdapter {
    fn source_id(&self) -> &'static str {
        "http-stream"
    }

    async fn search(
        &self,
        _query: &str,
        _context: &AuthContext,
    ) -> Result<Vec<TrackMetadata>, SourceError> {
        Ok(Vec::new())
    }

    async fn resolve(
        &self,
        track: &TrackReference,
        context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        if !context.allows("http.stream") {
            return Err(SourceError::Unauthorized);
        }
        validate_public_http_url(&track.query)?;
        Ok(ResolvedSource::Stream {
            url: track.query.clone(),
        })
    }

    async fn health(&self, _context: &AuthContext) -> Result<SourceHealth, SourceError> {
        Ok(SourceHealth::healthy(self.source_id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_prefers_exact_title_and_available_tracks() {
        let make = |id: &str, title: &str, available: bool| TrackMetadata {
            track_id: TrackId::new(id).unwrap(),
            title: title.to_string(),
            artist: None,
            duration_ms: None,
            source_id: "test".to_string(),
            available,
        };
        let ranked = rank_candidates(
            vec![
                make("a", "other", true),
                make("b", "song", false),
                make("c", "song", true),
            ],
            "song",
            None,
        );
        assert_eq!(ranked[0].track_id.as_str(), "c");
    }

    #[test]
    fn cancellation_handle_is_observable() {
        let (handle, token) = CancellationToken::new();
        assert!(!token.is_cancelled());
        handle.cancel();
        assert!(token.is_cancelled());
    }
}
