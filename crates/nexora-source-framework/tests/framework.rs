use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use nexora_source_api::{AudioSource, AuthContext, ResolvedSource, SourceError, TrackMetadata};
use nexora_source_framework::{SourceRegistry, validate_public_http_url};

fn auth() -> AuthContext {
    AuthContext::new("test-bot", vec!["media.local".into(), "http.stream".into()]).unwrap()
}

struct StubSource {
    id: &'static str,
    delay: std::time::Duration,
}

#[async_trait::async_trait]
impl AudioSource for StubSource {
    fn source_id(&self) -> &'static str {
        self.id
    }
    async fn search(
        &self,
        query: &str,
        _ctx: &AuthContext,
    ) -> Result<Vec<TrackMetadata>, SourceError> {
        tokio::time::sleep(self.delay).await;
        Ok(vec![TrackMetadata::local(
            "t1",
            query,
            PathBuf::from("/tmp/t.wav"),
        )])
    }
    async fn resolve(
        &self,
        _track: &nexora_source_api::TrackReference,
        _ctx: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        Ok(ResolvedSource::LocalFile {
            path: PathBuf::from("/tmp/t.wav"),
        })
    }
    async fn health(
        &self,
        _ctx: &AuthContext,
    ) -> Result<nexora_source_api::SourceHealth, SourceError> {
        Ok(nexora_source_api::SourceHealth::healthy(self.id))
    }
}

#[tokio::test]
async fn registry_searches_registered_sources() {
    let registry = SourceRegistry::new();
    registry
        .register(Box::new(StubSource {
            id: "stub",
            delay: std::time::Duration::ZERO,
        }))
        .unwrap();
    let results = registry.search("song", &auth()).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source_id, "local-media");
    assert_eq!(registry.source_ids(), vec!["stub"]);
}

struct FailingSource {
    id: &'static str,
    error: SourceError,
}

#[async_trait::async_trait]
impl AudioSource for FailingSource {
    fn source_id(&self) -> &'static str {
        self.id
    }
    async fn search(
        &self,
        _query: &str,
        _ctx: &AuthContext,
    ) -> Result<Vec<TrackMetadata>, SourceError> {
        Err(self.error.clone())
    }
    async fn resolve(
        &self,
        _track: &nexora_source_api::TrackReference,
        _ctx: &AuthContext,
    ) -> Result<ResolvedSource, SourceError> {
        Err(SourceError::NotFound)
    }
    async fn health(
        &self,
        _ctx: &AuthContext,
    ) -> Result<nexora_source_api::SourceHealth, SourceError> {
        Ok(nexora_source_api::SourceHealth::healthy(self.id))
    }
}

#[tokio::test]
async fn a_failing_source_does_not_fail_the_whole_search() {
    let registry = SourceRegistry::new();
    registry
        .register(Box::new(FailingSource {
            id: "broken-provider",
            error: SourceError::InvalidInput,
        }))
        .unwrap();
    registry
        .register(Box::new(FailingSource {
            id: "offline-provider",
            error: SourceError::Unavailable,
        }))
        .unwrap();
    registry
        .register(Box::new(StubSource {
            id: "stub",
            delay: std::time::Duration::ZERO,
        }))
        .unwrap();

    let results = registry.search("song", &auth()).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source_id, "local-media");
}

#[tokio::test]
async fn registry_enforces_timeout() {
    let registry = SourceRegistry::new();
    registry
        .register(Box::new(StubSource {
            id: "slow",
            delay: std::time::Duration::from_millis(200),
        }))
        .unwrap();
    let result = registry
        .search_with_timeout("song", &auth(), std::time::Duration::from_millis(10))
        .await;
    assert!(matches!(result, Err(SourceError::Timeout)));
}

#[test]
fn ssrf_validation_rejects_private_targets() {
    assert!(validate_public_http_url("https://example.com/stream").is_ok());
    assert!(matches!(
        validate_public_http_url("http://127.0.0.1/stream"),
        Err(SourceError::InvalidInput)
    ));
    assert!(matches!(
        validate_public_http_url("http://localhost/stream"),
        Err(SourceError::InvalidInput)
    ));
    assert!(matches!(
        validate_public_http_url("file:///etc/passwd"),
        Err(SourceError::InvalidInput)
    ));
}

#[test]
fn ssrf_validation_rejects_a_backslash_smuggled_private_target() {
    // A backslash ends the authority for an http(s) URL, so the host is what
    // precedes it. The validator must read the same host the fetcher will.
    for url in [
        "https://example.com\\@127.0.0.1/stream",
        "https://example.com\\169.254.169.254/stream",
        "https://example.com\\@localhost/stream",
    ] {
        assert!(
            matches!(
                validate_public_http_url(url),
                Err(SourceError::InvalidInput)
            ),
            "{url} must be rejected"
        );
    }
}

#[test]
fn ssrf_validation_rejects_a_smuggled_private_target() {
    for url in [
        "https://localhost?@example.com/stream",
        "https://2130706433/stream",
        "https://0x7f000001/stream",
        "https://127.1/stream",
        "https://127.0.0.1./stream",
        "https://[::ffff:127.0.0.1]/stream",
        "https://localhost./stream",
    ] {
        assert!(
            matches!(
                validate_public_http_url(url),
                Err(SourceError::InvalidInput)
            ),
            "{url} must be rejected"
        );
    }
}

#[test]
fn local_adapter_rejects_traversal() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("nexora-source-{unique}"));
    std::fs::create_dir_all(&root).unwrap();
    let adapter = nexora_source_framework::LocalMediaAdapter::new(root.clone()).unwrap();
    let ctx = auth();
    let outside = nexora_source_api::TrackReference::new("../outside.wav");
    let result = futures_executor_block_on(adapter.resolve(&outside, &ctx));
    assert!(matches!(result, Err(SourceError::InvalidInput)));
    let _ = std::fs::remove_dir_all(&root);
}

fn futures_executor_block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
