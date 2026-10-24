//! Spoken audio for short text through Google Translate's keyless text-to-speech endpoint.

use crate::error::ProviderError;

pub const DEFAULT_BASE_URL: &str = "https://translate.google.com";
/// The endpoint rejects longer input.
pub const MAX_CHARS: usize = 200;

pub struct GoogleSpeech {
    base_url: String,
    client: reqwest::Client,
}

impl GoogleSpeech {
    pub fn new(base_url: Option<String>) -> Self {
        Self { base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()), client: crate::providers::http_client() }
    }

    /// MP3 audio of `text` read in `lang`.
    pub async fn speak(&self, text: &str, lang: &str) -> Result<Vec<u8>, ProviderError> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > MAX_CHARS {
            return Err(ProviderError::Unsupported(format!("speech needs 1 to {MAX_CHARS} characters")));
        }
        let resp = self
            .client
            .get(format!("{}/translate_tts", self.base_url))
            .query(&[("ie", "UTF-8"), ("client", "tw-ob"), ("tl", lang), ("q", text)])
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ProviderError::from_response(resp).await);
        }
        Ok(resp.bytes().await?.to_vec())
    }
}
