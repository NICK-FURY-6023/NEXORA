use nexora_source_api::{SourceError, TrackMetadata};

use crate::{oauth::TokenStore, simple};

pub fn require_token(store: &TokenStore) -> Result<String, SourceError> {
    store.get("deezer").ok_or(SourceError::Unauthorized)
}

pub fn parse_search_response(body: &str) -> Result<Vec<TrackMetadata>, SourceError> {
    simple::parse(body, "deezer")
}
