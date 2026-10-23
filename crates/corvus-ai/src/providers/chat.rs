//! OpenAI-compatible `/chat/completions` streaming: Ollama, LM Studio, Groq, vLLM, ...
//! These servers have no built-in web search.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::AiError;
use crate::sse;
use crate::types::{AskEvent, AskReply, AskRequest, Assistant, AssistantInfo, ChatMessage, EventSink, Role, Usage};

pub struct ChatCompletions {
    id: String,
    name: String,
    base_url: String,
    api_key: Option<String>,
    model: String,
    client: reqwest::Client,
}

impl ChatCompletions {
    pub fn new(id: String, name: Option<String>, base_url: String, api_key: Option<String>, model: String) -> Self {
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

#[async_trait]
impl Assistant for ChatCompletions {
    fn info(&self) -> AssistantInfo {
        AssistantInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: "openai-compatible".into(),
            model: self.model.clone(),
            web_search: false,
        }
    }

    async fn ask(&self, req: &AskRequest, on_event: EventSink<'_>) -> Result<AskReply, AiError> {
        let mut messages = Vec::new();
        if let Some(system) = req.system.as_ref().filter(|s| !s.is_empty()) {
            messages.push(json!({ "role": "system", "content": system }));
        }
        for m in &req.messages {
            let role = if m.role == Role::User { "user" } else { "assistant" };
            messages.push(json!({ "role": role, "content": m.text }));
        }
        let body = json!({ "model": self.model, "messages": messages, "stream": true });
        let mut builder = self.client.post(format!("{}/chat/completions", self.base_url)).json(&body);
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }
        let resp = builder.send().await?;
        if !resp.status().is_success() {
            return Err(AiError::from_response(resp).await);
        }

        let mut text = String::new();
        let mut usage = None;
        sse::read_events(resp, |e| {
            if e.data.trim() == "[DONE]" {
                return Ok(false);
            }
            let data: Value = serde_json::from_str(&e.data).map_err(|err| AiError::Parse(err.to_string()))?;
            if let Some(msg) = data["error"]["message"].as_str() {
                return Err(AiError::Stream(msg.into()));
            }
            if let Some(delta) = data["choices"][0]["delta"]["content"].as_str() {
                text.push_str(delta);
                on_event(AskEvent::Text { delta: delta.into() });
            }
            if data["usage"].is_object() {
                usage = Some(Usage {
                    input_tokens: data["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
                    output_tokens: data["usage"]["completion_tokens"].as_u64().unwrap_or(0),
                });
            }
            Ok(true)
        })
        .await?;

        Ok(AskReply {
            message: ChatMessage { role: Role::Assistant, text, raw: None, provider: Some(self.id.clone()) },
            sources: Vec::new(),
            usage,
        })
    }
}
