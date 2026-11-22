use std::path::PathBuf;

use async_trait::async_trait;
use nexora_common::{ApplicationId, TimestampMs, TrackId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SourceError {
    #[error("source authorization failed")]
    Unauthorized,
    #[error("track not found")]
    NotFound,
    #[error("source unavailable")]
    Unavailable,
    #[error("invalid source input")]
    InvalidInput,
    #[error("source operation timed out")]
    Timeout,
    #[error("source rate limit reached")]
    RateLimited,
    #[error("source operation cancelled")]
    Cancelled,
    #[error("internal source error")]
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthContext {
    application: ApplicationId,
    scopes: Vec<String>,
}

impl AuthContext {
    pub fn new(application: impl Into<String>, scopes: Vec<String>) -> Result<Self, SourceError> {
        let application = ApplicationId::new(application).map_err(|_| SourceError::InvalidInput)?;
        Ok(Self {
            application,
            scopes,
        })
    }

    pub fn application(&self) -> &ApplicationId {
        &self.application
    }

    pub fn allows(&self, scope: &str) -> bool {
        self.scopes.iter().any(|value| value == scope)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackMetadata {
    pub track_id: TrackId,
    pub title: String,
    pub artist: Option<String>,
    pub duration_ms: Option<u64>,
    pub source_id: String,
    pub available: bool,
}

impl TrackMetadata {
    pub fn local(track_id: impl Into<String>, title: impl Into<String>, path: PathBuf) -> Self {
        let _ = path;
        Self {
            track_id: TrackId::new(track_id)
                .unwrap_or_else(|_| TrackId::new("unknown").expect("static id")),
            title: title.into(),
            artist: None,
            duration_ms: None,
            source_id: "local-media".to_string(),
            available: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackReference {
    pub query: String,
    pub source_hint: Option<String>,
}

impl TrackReference {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            source_hint: None,
        }
    }

    pub fn with_source_hint(mut self, source_id: impl Into<String>) -> Self {
        self.source_hint = Some(source_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedSource {
    LocalFile { path: PathBuf },
    Stream { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceHealth {
    pub source_id: String,
    pub healthy: bool,
    pub detail: String,
    pub checked_at: TimestampMs,
}

impl SourceHealth {
    pub fn healthy(source_id: impl Into<String>) -> Self {
        Self {
            source_id: source_id.into(),
            healthy: true,
            detail: "ok".to_string(),
            checked_at: TimestampMs::now(),
        }
    }

    pub fn unhealthy(source_id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            source_id: source_id.into(),
            healthy: false,
            detail: detail.into(),
            checked_at: TimestampMs::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCapabilities {
    pub search: bool,
    pub resolve: bool,
    pub health: bool,
}

impl SourceCapabilities {
    pub fn full() -> Self {
        Self {
            search: true,
            resolve: true,
            health: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRegistryEntry {
    pub source_id: &'static str,
    pub capabilities: SourceCapabilities,
}

#[async_trait]
pub trait AudioSource: Send + Sync {
    fn source_id(&self) -> &'static str;

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::full()
    }

    async fn search(
        &self,
        query: &str,
        context: &AuthContext,
    ) -> Result<Vec<TrackMetadata>, SourceError>;

    async fn resolve(
        &self,
        track: &TrackReference,
        context: &AuthContext,
    ) -> Result<ResolvedSource, SourceError>;

    async fn health(&self, context: &AuthContext) -> Result<SourceHealth, SourceError>;
}
