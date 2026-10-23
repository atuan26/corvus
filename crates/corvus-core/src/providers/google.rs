//! Google Translate through the keyless `translate_a/single` endpoint used by the web widget.
//! Unofficial: it can be rate-limited, so it should never be the only provider in the chain.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ProviderError;
use crate::provider::{ProviderInfo, TranslateRequest, Translation, Translator};

pub const DEFAULT_BASE_URL: &str = "https://translate.googleapis.com";

pub struct GoogleFree {
    id: String,
    base_url: String,
    client: reqwest::Client,
}

impl GoogleFree {
    pub fn new(id: impl Into<String>, base_url: Option<String>) -> Self {
        Self {
            id: id.into(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
            client: super::http_client(),
        }
    }
}

#[async_trait]
impl Translator for GoogleFree {
    fn info(&self) -> ProviderInfo {
        ProviderInfo { id: self.id.clone(), name: "Google (free)".into(), kind: "google-free".into() }
    }

    async fn translate(&self, req: &TranslateRequest) -> Result<Translation, ProviderError> {
        let source = if req.is_auto() { "auto" } else { req.source.as_str() };
        // POST keeps long inputs out of the URL, which the GET form truncates.
        let resp = self
            .client
            .post(format!("{}/translate_a/single", self.base_url))
            .query(&[("client", "gtx"), ("dt", "t"), ("ie", "UTF-8"), ("oe", "UTF-8")])
            .form(&[("sl", source), ("tl", req.target.as_str()), ("q", req.text.as_str())])
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ProviderError::from_response(resp).await);
        }
        let body: Value = resp.json().await?;
        parse(&self.id, &body)
    }
}

fn parse(id: &str, body: &Value) -> Result<Translation, ProviderError> {
    let segments = body
        .get(0)
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderError::Parse("missing sentence array".into()))?;
    let text: String = segments.iter().filter_map(|s| s.get(0).and_then(Value::as_str)).collect();
    let detected_source = body.get(2).and_then(Value::as_str).map(str::to_lowercase);
    Ok(Translation { text, detected_source, provider: id.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn joins_segments_and_reads_detected_language() {
        let body = json!([[["Xin chào. ", "Hello. ", null], ["Thế giới", "World", null]], null, "en"]);
        let t = parse("google", &body).unwrap();
        assert_eq!(t.text, "Xin chào. Thế giới");
        assert_eq!(t.detected_source.as_deref(), Some("en"));
    }

    #[test]
    fn rejects_unexpected_shape() {
        assert!(matches!(parse("google", &json!({"error": 1})), Err(ProviderError::Parse(_))));
    }
}
