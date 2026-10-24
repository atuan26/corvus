//! Any OpenAI-compatible `/chat/completions` endpoint: OpenAI, OpenRouter, Groq, Ollama,
//! LM Studio, vLLM. The model is prompted to return only the translation.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::ProviderError;
use crate::lang;
use crate::provider::{ProviderInfo, TranslateRequest, Translation, Translator};

pub struct OpenAiCompatible {
    id: String,
    name: String,
    base_url: String,
    api_key: Option<String>,
    model: String,
    client: reqwest::Client,
}

impl OpenAiCompatible {
    pub fn new(
        id: impl Into<String>,
        name: Option<String>,
        base_url: String,
        api_key: Option<String>,
        model: String,
    ) -> Self {
        let id = id.into();
        Self {
            name: name.unwrap_or_else(|| format!("AI ({model})")),
            id,
            base_url: base_url.trim_end_matches('/').into(),
            api_key,
            model,
            client: super::http_client(),
        }
    }
}

fn system_prompt(req: &TranslateRequest) -> String {
    let target = lang::name(&req.target).unwrap_or(&req.target);
    let source = if req.is_auto() {
        "the source language (detect it)".to_string()
    } else {
        lang::name(&req.source).unwrap_or(&req.source).to_string()
    };
    format!(
        "You are a professional translator. Translate the user's text from {source} into {target}. \
         Preserve meaning, tone, formatting and line breaks. Reply with the translation only, \
         without quotes, notes or explanations."
    )
}

#[derive(Deserialize)]
struct Response {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
}

#[async_trait]
impl Translator for OpenAiCompatible {
    fn info(&self) -> ProviderInfo {
        ProviderInfo { id: self.id.clone(), name: self.name.clone(), kind: "openai-compatible".into() }
    }

    async fn translate(&self, req: &TranslateRequest) -> Result<Translation, ProviderError> {
        let body = json!({
            "model": self.model,
            "temperature": 0.2,
            "messages": [
                { "role": "system", "content": system_prompt(req) },
                { "role": "user", "content": req.text },
            ],
        });
        let mut builder = self.client.post(format!("{}/chat/completions", self.base_url)).json(&body);
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }
        let resp = builder.send().await?;
        if !resp.status().is_success() {
            return Err(ProviderError::from_response(resp).await);
        }
        let parsed: Response = resp.json().await?;
        let text = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| ProviderError::Parse("no message content".into()))?;
        Ok(Translation { text: text.trim().into(), detected_source: None, provider: self.id.clone(), details: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_names_languages() {
        let p = system_prompt(&TranslateRequest::new("hi", "en", "vi"));
        assert!(p.contains("from English into Vietnamese"));
        let p = system_prompt(&TranslateRequest::new("hi", "auto", "ja"));
        assert!(p.contains("(detect it) into Japanese"));
    }
}
