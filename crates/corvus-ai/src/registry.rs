//! Build assistants from config. An assistant that can't run yet (not signed in, no key) is
//! reported with the reason so the UI can offer the fix.

use std::sync::Arc;

use corvus_core::{AssistantConfig, AssistantKind, SecretStore};

use crate::chatgpt::{ChatGptAuth, Endpoints, Login, Tokens};
use crate::providers::{chat::ChatCompletions, claude::Claude, responses};
use crate::types::Assistant;

pub const OPENAI_DEFAULT_MODEL: &str = "gpt-5.5";

pub fn default_name(kind: AssistantKind) -> &'static str {
    match kind {
        AssistantKind::Chatgpt => "ChatGPT",
        AssistantKind::Anthropic => "Claude",
        AssistantKind::Openai => "OpenAI API",
        AssistantKind::OpenaiCompatible => "Local AI",
    }
}

fn non_empty(s: &Option<String>) -> Option<String> {
    s.as_ref().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Blocking: may read the keychain.
pub fn build(cfg: &AssistantConfig, secrets: Arc<dyn SecretStore>, endpoints: &Endpoints) -> Result<Arc<dyn Assistant>, String> {
    let name = non_empty(&cfg.name).unwrap_or_else(|| default_name(cfg.kind).into());
    Ok(match cfg.kind {
        AssistantKind::Chatgpt => {
            let tokens = Tokens::load(secrets.as_ref())?.ok_or("Not signed in to ChatGPT")?;
            // Empty: the account's best model, looked up on first use.
            let model = non_empty(&cfg.model).unwrap_or_default();
            let auth = ChatGptAuth::new(Login::new(endpoints.clone()), tokens, secrets);
            Arc::new(responses::Responses::new(cfg.id.clone(), name, endpoints.backend_base.clone(), model, cfg.web_search, Box::new(auth)))
        }
        AssistantKind::Anthropic => {
            let key = cfg.resolve_key(secrets.as_ref()).0.ok_or("No Anthropic API key")?;
            Arc::new(Claude::new(cfg.id.clone(), Some(name), non_empty(&cfg.base_url), key, non_empty(&cfg.model), cfg.web_search))
        }
        AssistantKind::Openai => {
            let key = cfg.resolve_key(secrets.as_ref()).0.ok_or("No OpenAI API key")?;
            let base = non_empty(&cfg.base_url).unwrap_or_else(|| responses::OPENAI_BASE_URL.into());
            let model = non_empty(&cfg.model).unwrap_or_else(|| OPENAI_DEFAULT_MODEL.into());
            Arc::new(responses::Responses::new(cfg.id.clone(), name, base, model, cfg.web_search, Box::new(responses::ApiKeyAuth(key))))
        }
        AssistantKind::OpenaiCompatible => {
            let base = non_empty(&cfg.base_url).ok_or("Base URL is missing")?;
            let model = non_empty(&cfg.model).ok_or("Model is missing")?;
            let key = cfg.resolve_key(secrets.as_ref()).0;
            Arc::new(ChatCompletions::new(cfg.id.clone(), Some(name), base, key, model))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvus_core::MemoryStore;

    #[test]
    fn reports_why_an_assistant_is_not_ready() {
        let secrets: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
        let e = Endpoints::default();
        let err = |cfg| build(&cfg, secrets.clone(), &e).err().unwrap();
        assert_eq!(err(AssistantConfig::new("chatgpt", AssistantKind::Chatgpt)), "Not signed in to ChatGPT");
        assert_eq!(err(AssistantConfig::new("claude", AssistantKind::Anthropic)), "No Anthropic API key");
        assert_eq!(err(AssistantConfig::new("local", AssistantKind::OpenaiCompatible)), "Base URL is missing");

        secrets.set("claude", "sk-ant").unwrap();
        let claude = build(&AssistantConfig::new("claude", AssistantKind::Anthropic), secrets.clone(), &e).unwrap();
        assert_eq!(claude.info().model, "claude-opus-5-5");
        assert_eq!(claude.info().name, "Claude");
    }
}
