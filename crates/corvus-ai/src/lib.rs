//! Streaming AI assistants for the Ask feature: Claude (Anthropic API), OpenAI-style Responses
//! (API key or ChatGPT sign-in) and OpenAI-compatible chat completions (Ollama, Groq, ...).

pub mod chatgpt;
pub mod error;
pub mod providers;
pub mod registry;
pub mod sse;
pub mod types;

pub use error::AiError;
pub use types::{AskEvent, AskReply, AskRequest, Assistant, AssistantInfo, ChatMessage, Role, Source, Usage};
