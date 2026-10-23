//! Claude via the Anthropic Messages API (raw HTTP; there is no official Rust SDK).
//! Streams text, runs Anthropic's server-side web search, and keeps the full content blocks of
//! each answer so follow-ups replay them unchanged (thinking blocks must not be edited).

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::AiError;
use crate::sse;
use crate::types::{AskEvent, AskReply, AskRequest, Assistant, AssistantInfo, ChatMessage, EventSink, Role, SourceList, Usage};

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
const API_VERSION: &str = "2026-10-21";
const WEB_SEARCH_TOOL: &str = "web_search_20260209";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Server-side tool loops can pause; resume a few times before giving up.
const MAX_CONTINUATIONS: usize = 3;

pub struct Claude {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    model: String,
    web_search: bool,
    client: reqwest::Client,
}

impl Claude {
    pub fn new(id: String, name: Option<String>, base_url: Option<String>, api_key: String, model: Option<String>, web_search: bool) -> Self {
        let model = model.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| DEFAULT_MODEL.into());
        Self {
            name: name.unwrap_or_else(|| "Claude".into()),
            id,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()).trim_end_matches('/').into(),
            api_key,
            model,
            web_search,
            client: super::http_client(),
        }
    }

    /// Refusal fallbacks re-run a declined request on another model inside the same call.
    /// Only the first-party API and these models accept the `"default"` form.
    fn use_fallbacks(&self) -> bool {
        self.base_url == DEFAULT_BASE_URL
            && matches!(self.model.as_str(), "claude-opus-5-5" | "claude-opus-5" | "claude-sonnet-5-5" | "claude-fable-5-1")
    }

    fn messages(&self, history: &[ChatMessage]) -> Vec<Value> {
        history
            .iter()
            .map(|m| match m.role {
                Role::User => json!({ "role": "user", "content": m.text }),
                Role::Assistant => match m.raw_for(&self.id) {
                    Some(blocks) => json!({ "role": "assistant", "content": blocks }),
                    None => json!({ "role": "assistant", "content": m.text }),
                },
            })
            .collect()
    }

    fn body(&self, req: &AskRequest, messages: &[Value]) -> Value {
        let mut body = json!({
            "model": self.model,
            "max_tokens": 64000,
            "stream": true,
            "messages": messages,
        });
        if let Some(system) = req.system.as_ref().filter(|s| !s.is_empty()) {
            body["system"] = system.clone().into();
        }
        if req.web_search && self.web_search {
            body["tools"] = json!([{ "type": WEB_SEARCH_TOOL, "name": "web_search", "max_uses": 5 }]);
        }
        if self.use_fallbacks() {
            body["fallbacks"] = "default".into();
        }
        body
    }
}

/// One streamed response, rebuilt into the content blocks the API would have returned.
#[derive(Default)]
struct Turn {
    blocks: Vec<Value>,
    partial_json: Vec<String>,
    stop_reason: Option<String>,
    usage: Usage,
}

impl Turn {
    fn handle(&mut self, data: &Value, sources: &mut SourceList, on_event: EventSink<'_>) -> Result<(), AiError> {
        let index = data["index"].as_u64().unwrap_or(0) as usize;
        match data["type"].as_str().unwrap_or_default() {
            "message_start" => {
                self.usage.input_tokens = data["message"]["usage"]["input_tokens"].as_u64().unwrap_or(0);
            }
            "content_block_start" => {
                let block = data["content_block"].clone();
                if block["type"] == "web_search_tool_result" {
                    // A list on success; an error object otherwise.
                    for r in block["content"].as_array().into_iter().flatten() {
                        sources.add(r["url"].as_str().unwrap_or_default(), r["title"].as_str(), on_event);
                    }
                }
                while self.blocks.len() <= index {
                    self.blocks.push(Value::Null);
                    self.partial_json.push(String::new());
                }
                self.blocks[index] = block;
            }
            "content_block_delta" => {
                let Some(block) = self.blocks.get_mut(index) else { return Ok(()) };
                let delta = &data["delta"];
                match delta["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        let text = delta["text"].as_str().unwrap_or_default();
                        append(block, "text", text);
                        on_event(AskEvent::Text { delta: text.into() });
                    }
                    "thinking_delta" => append(block, "thinking", delta["thinking"].as_str().unwrap_or_default()),
                    "signature_delta" => block["signature"] = delta["signature"].clone(),
                    "input_json_delta" => self.partial_json[index].push_str(delta["partial_json"].as_str().unwrap_or_default()),
                    "citations_delta" => {
                        let citation = delta["citation"].clone();
                        if let Some(url) = citation["url"].as_str() {
                            sources.add(url, citation["title"].as_str(), on_event);
                        }
                        if !block["citations"].is_array() {
                            block["citations"] = json!([]);
                        }
                        block["citations"].as_array_mut().unwrap().push(citation);
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                let Some(block) = self.blocks.get_mut(index) else { return Ok(()) };
                let partial = std::mem::take(&mut self.partial_json[index]);
                if !partial.is_empty() {
                    block["input"] = serde_json::from_str(&partial).unwrap_or(Value::Null);
                }
                if block["type"] == "server_tool_use" && block["name"] == "web_search" {
                    if let Some(q) = block["input"]["query"].as_str() {
                        on_event(AskEvent::Searching { query: q.into() });
                    }
                }
            }
            "message_delta" => {
                if let Some(r) = data["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(r.into());
                }
                if let Some(n) = data["usage"]["output_tokens"].as_u64() {
                    self.usage.output_tokens = n;
                }
            }
            "error" => {
                let msg = data["error"]["message"].as_str().unwrap_or("stream error");
                return Err(AiError::Stream(msg.into()));
            }
            _ => {}
        }
        Ok(())
    }
}

fn append(block: &mut Value, field: &str, text: &str) {
    let current = block[field].as_str().unwrap_or_default();
    block[field] = format!("{current}{text}").into();
}

#[async_trait]
impl Assistant for Claude {
    fn info(&self) -> AssistantInfo {
        AssistantInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: "anthropic".into(),
            model: self.model.clone(),
            web_search: self.web_search,
        }
    }

    async fn ask(&self, req: &AskRequest, on_event: EventSink<'_>) -> Result<AskReply, AiError> {
        let mut messages = self.messages(&req.messages);
        let mut all_blocks: Vec<Value> = Vec::new();
        let mut usage = Usage::default();
        let mut sources = SourceList::new();

        for _ in 0..=MAX_CONTINUATIONS {
            let mut builder = self
                .client
                .post(format!("{}/v1/messages", self.base_url))
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", API_VERSION)
                .json(&self.body(req, &messages));
            if self.use_fallbacks() {
                builder = builder.header("anthropic-beta", FALLBACK_BETA);
            }
            let resp = builder.send().await?;
            if !resp.status().is_success() {
                return Err(AiError::from_response(resp).await);
            }
            let mut turn = Turn::default();
            sse::read_events(resp, |e| {
                let data: Value = serde_json::from_str(&e.data).map_err(|err| AiError::Parse(err.to_string()))?;
                turn.handle(&data, &mut sources, on_event)?;
                Ok(true)
            })
            .await?;

            usage.input_tokens += turn.usage.input_tokens;
            usage.output_tokens += turn.usage.output_tokens;
            let blocks: Vec<Value> = turn.blocks.into_iter().filter(|b| !b.is_null()).collect();
            all_blocks.extend(blocks.iter().cloned());
            match turn.stop_reason.as_deref() {
                // The server-side tool loop paused: send the partial turn back and it resumes.
                Some("pause_turn") => messages.push(json!({ "role": "assistant", "content": blocks })),
                Some("refusal") => return Err(AiError::Refused(String::new())),
                _ => break,
            }
        }

        let text: String = all_blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect();
        Ok(AskReply {
            message: ChatMessage {
                role: Role::Assistant,
                text,
                raw: Some(Value::Array(all_blocks)),
                provider: Some(self.id.clone()),
            },
            sources: sources.into_vec(),
            usage: Some(usage),
        })
    }
}
