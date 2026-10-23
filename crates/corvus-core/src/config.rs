use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::ProviderError;
use crate::provider::Translator;
use crate::providers::{deepl::DeepL, google::GoogleFree, openai::OpenAiCompatible};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub default_source: String,
    pub default_target: String,
    /// Per-provider attempt timeout before the router falls back to the next one.
    pub timeout_secs: u64,
    /// Fallback order, by provider id.
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Literal key. Prefer `api_key_env` so keys stay out of the config file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    GoogleFree,
    Deepl,
    OpenaiCompatible,
}

fn yes() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_source: "auto".into(),
            default_target: "en".into(),
            timeout_secs: 15,
            providers: vec![
                ProviderConfig::new("google", ProviderKind::GoogleFree),
                ProviderConfig {
                    enabled: false,
                    api_key_env: Some("DEEPL_API_KEY".into()),
                    ..ProviderConfig::new("deepl", ProviderKind::Deepl)
                },
                ProviderConfig {
                    name: Some("Ollama".into()),
                    enabled: false,
                    base_url: Some("http://localhost:11434/v1".into()),
                    model: Some("llama3.2".into()),
                    ..ProviderConfig::new("ollama", ProviderKind::OpenaiCompatible)
                },
            ],
        }
    }
}

impl ProviderConfig {
    pub fn new(id: &str, kind: ProviderKind) -> Self {
        Self {
            id: id.into(),
            kind,
            enabled: true,
            name: None,
            base_url: None,
            model: None,
            api_key: None,
            api_key_env: None,
        }
    }

    fn api_key(&self) -> Option<String> {
        self.api_key
            .clone()
            .or_else(|| self.api_key_env.as_ref().and_then(|var| std::env::var(var).ok()))
            .filter(|k| !k.trim().is_empty())
    }

    pub fn build(&self) -> Result<Arc<dyn Translator>, ProviderError> {
        let id = self.id.clone();
        Ok(match self.kind {
            ProviderKind::GoogleFree => Arc::new(GoogleFree::new(id, self.base_url.clone())),
            ProviderKind::Deepl => {
                let key = self.api_key().ok_or_else(|| {
                    ProviderError::Config(format!("{id}: missing API key (set api_key or api_key_env)"))
                })?;
                Arc::new(DeepL::new(id, key, self.base_url.clone()))
            }
            ProviderKind::OpenaiCompatible => {
                let base_url = self
                    .base_url
                    .clone()
                    .ok_or_else(|| ProviderError::Config(format!("{id}: missing base_url")))?;
                let model =
                    self.model.clone().ok_or_else(|| ProviderError::Config(format!("{id}: missing model")))?;
                Arc::new(OpenAiCompatible::new(id, self.name.clone(), base_url, self.api_key(), model))
            }
        })
    }
}

impl Config {
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("corvus").join("config.toml"))
    }

    /// Load the config, writing the defaults on first run so users have a file to edit.
    pub fn load_or_init(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let config = Self::default();
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let text = toml::to_string_pretty(&config).map_err(|e| e.to_string())?;
                std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
                Ok(config)
            }
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_round_trips() {
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.providers.len(), 3);
        assert_eq!(back.providers[2].kind, ProviderKind::OpenaiCompatible);
        assert!(!back.providers[2].enabled);
    }

    #[test]
    fn deepl_without_key_is_a_config_error() {
        let p = ProviderConfig::new("deepl", ProviderKind::Deepl);
        assert!(matches!(p.build(), Err(ProviderError::Config(_))));
    }
}
