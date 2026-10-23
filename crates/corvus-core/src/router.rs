use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::config::Config;
use crate::error::ProviderError;
use crate::provider::{ProviderInfo, TranslateRequest, Translation, Translator};
use crate::secrets::SecretStore;

/// Tries providers in order until one succeeds, so a single dead service never blocks a translation.
pub struct Router {
    providers: Vec<Arc<dyn Translator>>,
    timeout: Duration,
}

#[derive(Debug, Clone, Serialize)]
pub struct Attempt {
    pub provider: String,
    pub error: String,
}

#[derive(Debug, thiserror::Error, Serialize)]
pub enum RouterError {
    #[error("no providers are enabled")]
    NoProviders,
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("nothing to translate")]
    EmptyInput,
    #[error("all providers failed")]
    AllFailed(Vec<Attempt>),
}

impl Router {
    pub fn new(providers: Vec<Arc<dyn Translator>>, timeout: Duration) -> Self {
        Self { providers, timeout }
    }

    /// Build from config. Misconfigured providers are skipped and reported as warnings.
    pub fn from_config(config: &Config, secrets: &dyn SecretStore) -> (Self, Vec<String>) {
        let mut providers = Vec::new();
        let mut warnings = Vec::new();
        for p in config.providers.iter().filter(|p| p.enabled) {
            match p.build(secrets) {
                Ok(t) => providers.push(t),
                Err(e) => warnings.push(e.to_string()),
            }
        }
        (Self::new(providers, Duration::from_secs(config.timeout_secs.max(1))), warnings)
    }

    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.providers.iter().map(|p| p.info()).collect()
    }

    /// Translate with `preferred` first (if given), then the rest in configured order.
    pub async fn translate(
        &self,
        req: &TranslateRequest,
        preferred: Option<&str>,
    ) -> Result<Translation, RouterError> {
        if req.text.trim().is_empty() {
            return Err(RouterError::EmptyInput);
        }
        if self.providers.is_empty() {
            return Err(RouterError::NoProviders);
        }
        let mut order: Vec<&Arc<dyn Translator>> = self.providers.iter().collect();
        if let Some(id) = preferred {
            let pos = order
                .iter()
                .position(|p| p.info().id == id)
                .ok_or_else(|| RouterError::UnknownProvider(id.into()))?;
            let first = order.remove(pos);
            order.insert(0, first);
        }

        let mut attempts = Vec::new();
        for provider in order {
            let id = provider.info().id;
            let result = match tokio::time::timeout(self.timeout, provider.translate(req)).await {
                Ok(r) => r,
                Err(_) => Err(ProviderError::Timeout(self.timeout)),
            };
            match result {
                Ok(t) => return Ok(t),
                Err(e) => {
                    tracing::warn!(provider = %id, error = %e, "translation failed, trying next provider");
                    attempts.push(Attempt { provider: id, error: e.to_string() });
                }
            }
        }
        Err(RouterError::AllFailed(attempts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct Fake {
        id: &'static str,
        result: Result<&'static str, u16>,
        delay: Duration,
    }

    #[async_trait]
    impl Translator for Fake {
        fn info(&self) -> ProviderInfo {
            ProviderInfo { id: self.id.into(), name: self.id.into(), kind: "fake".into() }
        }

        async fn translate(&self, _: &TranslateRequest) -> Result<Translation, ProviderError> {
            tokio::time::sleep(self.delay).await;
            match self.result {
                Ok(text) => Ok(Translation { text: text.into(), detected_source: None, provider: self.id.into() }),
                Err(status) => Err(ProviderError::Http { status, body: String::new() }),
            }
        }
    }

    fn fake(id: &'static str, result: Result<&'static str, u16>) -> Arc<dyn Translator> {
        Arc::new(Fake { id, result, delay: Duration::ZERO })
    }

    fn req() -> TranslateRequest {
        TranslateRequest::new("hello", "auto", "vi")
    }

    #[tokio::test]
    async fn falls_back_on_error() {
        let r = Router::new(vec![fake("a", Err(503)), fake("b", Ok("xin chào"))], Duration::from_secs(1));
        let t = r.translate(&req(), None).await.unwrap();
        assert_eq!(t.provider, "b");
    }

    #[tokio::test]
    async fn falls_back_on_timeout() {
        let slow = Arc::new(Fake { id: "slow", result: Ok("late"), delay: Duration::from_secs(5) });
        let r = Router::new(vec![slow, fake("b", Ok("fast"))], Duration::from_millis(50));
        assert_eq!(r.translate(&req(), None).await.unwrap().provider, "b");
    }

    #[tokio::test]
    async fn preferred_provider_goes_first() {
        let r = Router::new(vec![fake("a", Ok("A")), fake("b", Ok("B"))], Duration::from_secs(1));
        assert_eq!(r.translate(&req(), Some("b")).await.unwrap().provider, "b");
        assert!(matches!(r.translate(&req(), Some("zzz")).await, Err(RouterError::UnknownProvider(_))));
    }

    #[tokio::test]
    async fn reports_every_attempt_when_all_fail() {
        let r = Router::new(vec![fake("a", Err(500)), fake("b", Err(502))], Duration::from_secs(1));
        match r.translate(&req(), None).await {
            Err(RouterError::AllFailed(a)) => assert_eq!(a.len(), 2),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_blank_input() {
        let r = Router::new(vec![fake("a", Ok("A"))], Duration::from_secs(1));
        let blank = TranslateRequest::new("  ", "auto", "vi");
        assert!(matches!(r.translate(&blank, None).await, Err(RouterError::EmptyInput)));
    }
}
