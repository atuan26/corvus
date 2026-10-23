//! OpenAI Responses API streaming. Used with an API key against api.openai.com, and (through a
//! different `ResponsesAuth`) with a ChatGPT sign-in against the Codex backend.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::AiError;
use crate::sse;
use crate::types::{AskEvent, AskReply, AskRequest, Assistant, AssistantInfo, ChatMessage, EventSink, Role, SourceList, Usage};

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// How requests are authenticated and adjusted for a particular backend.
#[async_trait]
pub trait ResponsesAuth: Send + Sync {
    fn kind(&self) -> &'static str;
    /// Headers for one request; may refresh credentials first.
    async fn headers(&self) -> Result<Vec<(String, String)>, AiError>;
    /// Backend-specific body changes (e.g. required fields).
    fn adjust_body(&self, _body: &mut Value) {}
    /// The model to request. An empty `configured` value means "the backend's best".
    async fn resolve_model(&self, configured: &str) -> Result<String, AiError> {
        if configured.is_empty() {
            return Err(AiError::Config("no model set".into()));
        }
        Ok(configured.to_string())
    }
    /// Called on HTTP 401; return true after renewing credentials to retry once.
    async fn on_unauthorized(&self) -> Result<bool, AiError> {
        Ok(false)
    }
}

pub struct ApiKeyAuth(pub String);

#[async_trait]
impl ResponsesAuth for ApiKeyAuth {
    fn kind(&self) -> &'static str {
        "openai"
    }

    async fn headers(&self) -> Result<Vec<(String, String)>, AiError> {
        Ok(vec![("Authorization".into(), format!("Bearer {}", self.0))])
    }
}

pub struct Responses {
    id: String,
    name: String,
    base_url: String,
    model: String,
    web_search: bool,
    auth: Box<dyn ResponsesAuth>,
    client: reqwest::Client,
}

impl Responses {
    pub fn new(
        id: String,
        name: String,
        base_url: String,
        model: String,
        web_search: bool,
        auth: Box<dyn ResponsesAuth>,
    ) -> Self {
        Self {
            id,
            name,
            base_url: base_url.trim_end_matches('/').into(),
            model,
            web_search,
            auth,
            client: super::http_client(),
        }
    }

    pub fn body(&self, req: &AskRequest, model: &str) -> Value {
        // Conversation state is resent as text each turn (`store: false` keeps nothing server-side).
        let input: Vec<Value> = req
            .messages
            .iter()
            .map(|m| match m.role {
                Role::User => json!({ "role": "user", "content": [{ "type": "input_text", "text": m.text }] }),
                Role::Assistant => json!({ "role": "assistant", "content": [{ "type": "output_text", "text": m.text }] }),
            })
            .collect();
        let mut body = json!({
            "model": model,
            "input": input,
            "stream": true,
            "store": false,
        });
        if let Some(system) = req.system.as_ref().filter(|s| !s.is_empty()) {
            body["instructions"] = system.clone().into();
        }
        if req.web_search && self.web_search {
            body["tools"] = json!([{ "type": "web_search" }]);
        }
        self.auth.adjust_body(&mut body);
        body
    }
}

fn add_annotations(content: &Value, sources: &mut SourceList, on_event: EventSink<'_>) {
    for a in content["annotations"].as_array().into_iter().flatten() {
        if a["type"] == "url_citation" {
            sources.add(a["url"].as_str().unwrap_or_default(), a["title"].as_str(), on_event);
        }
    }
}

#[async_trait]
impl Assistant for Responses {
    fn info(&self) -> AssistantInfo {
        AssistantInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.auth.kind().into(),
            // Empty means it is picked from the account's models on first use.
            model: if self.model.is_empty() { "best available".into() } else { self.model.clone() },
            web_search: self.web_search,
        }
    }

    async fn ask(&self, req: &AskRequest, on_event: EventSink<'_>) -> Result<AskReply, AiError> {
        let model = self.auth.resolve_model(&self.model).await?;
        let body = self.body(req, &model);
        let mut retried = false;
        let resp = loop {
            let mut builder = self.client.post(format!("{}/responses", self.base_url)).json(&body);
            for (k, v) in self.auth.headers().await? {
                builder = builder.header(k, v);
            }
            let resp = builder.header("Accept", "text/event-stream").send().await?;
            if resp.status().as_u16() == 401 && !retried && self.auth.on_unauthorized().await? {
                retried = true;
                continue;
            }
            if !resp.status().is_success() {
                return Err(AiError::from_response(resp).await);
            }
            break resp;
        };

        let mut text = String::new();
        let mut sources = SourceList::new();
        let mut usage = None;
        sse::read_events(resp, |e| {
            if e.data == "[DONE]" {
                return Ok(false);
            }
            let data: Value = serde_json::from_str(&e.data).map_err(|err| AiError::Parse(err.to_string()))?;
            match data["type"].as_str().unwrap_or_default() {
                "response.output_text.delta" => {
                    let delta = data["delta"].as_str().unwrap_or_default();
                    text.push_str(delta);
                    on_event(AskEvent::Text { delta: delta.into() });
                }
                "response.output_text.annotation.added" => {
                    let a = &data["annotation"];
                    if a["type"] == "url_citation" {
                        sources.add(a["url"].as_str().unwrap_or_default(), a["title"].as_str(), on_event);
                    }
                }
                "response.output_item.done" => {
                    let item = &data["item"];
                    if item["type"] == "web_search_call" {
                        if let Some(q) = item["action"]["query"].as_str() {
                            on_event(AskEvent::Searching { query: q.into() });
                        }
                    }
                    for c in item["content"].as_array().into_iter().flatten() {
                        add_annotations(c, &mut sources, on_event);
                    }
                }
                "response.completed" | "response.incomplete" => {
                    let u = &data["response"]["usage"];
                    usage = Some(Usage {
                        input_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                        output_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                    });
                    return Ok(false);
                }
                "response.failed" => {
                    let msg = data["response"]["error"]["message"].as_str().unwrap_or("the request failed");
                    return Err(AiError::Stream(msg.into()));
                }
                "error" => {
                    let msg = data["message"].as_str().or(data["error"]["message"].as_str()).unwrap_or("stream error");
                    return Err(AiError::Stream(msg.into()));
                }
                _ => {}
            }
            Ok(true)
        })
        .await?;

        Ok(AskReply {
            message: ChatMessage { role: Role::Assistant, text, raw: None, provider: Some(self.id.clone()) },
            sources: sources.into_vec(),
            usage,
        })
    }
}
