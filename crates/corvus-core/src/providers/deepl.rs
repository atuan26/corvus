//! DeepL API v2. Free-tier keys end in `:fx` and must use the `api-free` host.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::ProviderError;
use crate::provider::{ProviderInfo, TranslateRequest, Translation, Translator};

pub struct DeepL {
    id: String,
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl DeepL {
    pub fn new(id: impl Into<String>, api_key: String, base_url: Option<String>) -> Self {
        let base_url = base_url.unwrap_or_else(|| {
            if api_key.ends_with(":fx") { "https://api-free.deepl.com" } else { "https://api.deepl.com" }.into()
        });
        Self { id: id.into(), api_key, base_url, client: super::http_client() }
    }
}

/// DeepL wants upper-case codes and a regional variant for English and Portuguese targets.
fn target_code(code: &str) -> String {
    match code.to_ascii_lowercase().as_str() {
        "en" => "EN-US".into(),
        "pt" => "PT-BR".into(),
        "zh" | "zh-cn" => "ZH-HANS".into(),
        "zh-tw" => "ZH-HANT".into(),
        other => other.to_ascii_uppercase(),
    }
}

/// Source codes have no regional variants.
fn source_code(code: &str) -> String {
    code.split('-').next().unwrap_or(code).to_ascii_uppercase()
}

#[derive(Deserialize)]
struct Response {
    translations: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    text: String,
    detected_source_language: Option<String>,
}

#[async_trait]
impl Translator for DeepL {
    fn info(&self) -> ProviderInfo {
        ProviderInfo { id: self.id.clone(), name: "DeepL".into(), kind: "deepl".into() }
    }

    async fn translate(&self, req: &TranslateRequest) -> Result<Translation, ProviderError> {
        let mut body = json!({ "text": [req.text], "target_lang": target_code(&req.target) });
        if !req.is_auto() {
            body["source_lang"] = source_code(&req.source).into();
        }
        let resp = self
            .client
            .post(format!("{}/v2/translate", self.base_url))
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ProviderError::from_response(resp).await);
        }
        let parsed: Response = resp.json().await?;
        let item = parsed
            .translations
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::Parse("empty translations array".into()))?;
        Ok(Translation {
            text: item.text,
            detected_source: item.detected_source_language.map(|l| l.to_lowercase()),
            provider: self.id.clone(),
            details: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_language_codes() {
        assert_eq!(target_code("en"), "EN-US");
        assert_eq!(target_code("vi"), "VI");
        assert_eq!(target_code("zh-TW"), "ZH-HANT");
        assert_eq!(source_code("zh-CN"), "ZH");
    }

    #[test]
    fn picks_free_host_for_fx_keys() {
        assert_eq!(DeepL::new("d", "abc:fx".into(), None).base_url, "https://api-free.deepl.com");
        assert_eq!(DeepL::new("d", "abc".into(), None).base_url, "https://api.deepl.com");
    }
}
