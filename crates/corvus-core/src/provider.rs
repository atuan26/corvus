use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::dictionary::Details;
use crate::error::ProviderError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    /// ISO 639-1 style code, or `"auto"` to let the provider detect it.
    #[serde(default = "auto")]
    pub source: String,
    pub target: String,
}

fn auto() -> String {
    "auto".into()
}

impl TranslateRequest {
    pub fn new(text: impl Into<String>, source: impl Into<String>, target: impl Into<String>) -> Self {
        Self { text: text.into(), source: source.into(), target: target.into() }
    }

    pub fn is_auto(&self) -> bool {
        self.source.is_empty() || self.source == "auto"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Translation {
    pub text: String,
    /// Source language reported by the provider, lower-cased.
    pub detected_source: Option<String>,
    /// Id of the provider that produced this result.
    pub provider: String,
    /// Dictionary extras (pronunciation, meanings, examples), when the provider has them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Details>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[async_trait]
pub trait Translator: Send + Sync {
    fn info(&self) -> ProviderInfo;

    async fn translate(&self, req: &TranslateRequest) -> Result<Translation, ProviderError>;
}
