use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub text: String,
    /// Provider-native content of an assistant turn (e.g. Claude content blocks with thinking,
    /// tool use and citations), replayed verbatim to the same provider so nothing is lost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
    /// Id of the assistant that produced `raw`; other assistants get `text` instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: Role::User, text: text.into(), raw: None, provider: None }
    }

    /// The raw content to send to `provider_id`, if this turn came from it.
    pub fn raw_for(&self, provider_id: &str) -> Option<&Value> {
        self.raw.as_ref().filter(|_| self.provider.as_deref() == Some(provider_id))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskRequest {
    #[serde(default)]
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub web_search: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Incremental updates while an answer streams in.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AskEvent {
    Text { delta: String },
    Searching { query: String },
    Source { source: Source },
}

/// The complete answer, ready to append to the conversation.
#[derive(Debug, Clone, Serialize)]
pub struct AskReply {
    pub message: ChatMessage,
    pub sources: Vec<Source>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub model: String,
    pub web_search: bool,
}

pub type EventSink<'a> = &'a (dyn Fn(AskEvent) + Send + Sync);

#[async_trait]
pub trait Assistant: Send + Sync {
    fn info(&self) -> AssistantInfo;

    /// Stream an answer, reporting progress through `on_event`. Cancel by dropping the future.
    async fn ask(&self, req: &AskRequest, on_event: EventSink<'_>) -> Result<AskReply, AiError>;
}

/// Collects sources without duplicates, forwarding new ones to the UI.
pub(crate) struct SourceList(Vec<Source>);

impl SourceList {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn add(&mut self, url: &str, title: Option<&str>, on_event: EventSink<'_>) {
        if url.is_empty() || self.0.iter().any(|s| s.url == url) {
            return;
        }
        let source = Source { url: url.into(), title: title.filter(|t| !t.is_empty()).map(Into::into) };
        on_event(AskEvent::Source { source: source.clone() });
        self.0.push(source);
    }

    pub fn into_vec(self) -> Vec<Source> {
        self.0
    }
}
