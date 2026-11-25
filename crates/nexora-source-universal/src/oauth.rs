use std::collections::HashMap;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use nexora_source_api::SourceError;
use rand::RngCore;
use sha2::{Digest, Sha256};

#[derive(Default)]
pub struct TokenStore {
    tokens: HashMap<String, String>,
}

fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

impl TokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, provider: &str) -> Option<String> {
        self.tokens.get(provider).cloned()
    }

    pub fn put(&mut self, provider: &str, token: String) {
        self.tokens.insert(provider.to_owned(), token);
    }

    pub fn authorization_url(
        &self,
        provider: &str,
        client_id: &str,
        redirect_uri: &str,
    ) -> Result<(String, String), SourceError> {
        if provider.is_empty() || client_id.is_empty() || redirect_uri.is_empty() {
            return Err(SourceError::InvalidInput);
        }
        let mut random = [0u8; 32];
        rand::rng().fill_bytes(&mut random);
        let verifier = URL_SAFE_NO_PAD.encode(random);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let url = format!(
            "https://accounts.{provider}.com/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256",
            encode(client_id),
            encode(redirect_uri),
            encode(&challenge)
        );
        Ok((url, verifier))
    }
}
