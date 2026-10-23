use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::ProviderError;
use crate::provider::Translator;
use crate::providers::{deepl::DeepL, google::GoogleFree, openai::OpenAiCompatible};
use crate::secrets::SecretStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub default_source: String,
    pub default_target: String,
    /// Per-provider attempt timeout before the router falls back to the next one.
    pub timeout_secs: u64,
    /// Global shortcuts used by the desktop app.
    pub shortcuts: Shortcuts,
    /// Fallback order, by provider id.
    pub providers: Vec<ProviderConfig>,
}

/// Key sequences such as `"Ctrl+Alt+E"`. An empty string disables that shortcut.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Shortcuts {
    pub show_window: String,
    pub translate_selection: String,
    pub replace_selection: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            show_window: "Ctrl+Alt+C".into(),
            translate_selection: "Ctrl+Alt+E".into(),
            replace_selection: "Ctrl+Alt+R".into(),
        }
    }
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
    /// Literal key, used only when the keychain is unavailable. Normally the key is in the
    /// OS keychain under service `corvus`, account = provider id.
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

impl ProviderKind {
    pub fn requires_key(self) -> bool {
        matches!(self, Self::Deepl)
    }
}

/// Where a provider's API key was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeySource {
    Config,
    Keychain,
    Env,
    None,
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
            shortcuts: Shortcuts::default(),
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

    /// Resolve the API key: config file, then keychain, then environment variable.
    /// A keychain failure is logged and treated as "not there" so the env var still works.
    pub fn resolve_key(&self, secrets: &dyn SecretStore) -> (Option<String>, KeySource) {
        let usable = |k: &String| !k.trim().is_empty();
        if let Some(k) = self.api_key.clone().filter(usable) {
            return (Some(k), KeySource::Config);
        }
        match secrets.get(&self.id) {
            Ok(Some(k)) if usable(&k) => return (Some(k), KeySource::Keychain),
            Ok(_) => {}
            Err(e) => tracing::warn!(provider = %self.id, "{e}"),
        }
        if let Some(k) = self.api_key_env.as_ref().and_then(|var| std::env::var(var).ok()).filter(usable) {
            return (Some(k), KeySource::Env);
        }
        (None, KeySource::None)
    }

    pub fn build(&self, secrets: &dyn SecretStore) -> Result<Arc<dyn Translator>, ProviderError> {
        let id = self.id.clone();
        let key = match self.kind {
            ProviderKind::GoogleFree => None,
            _ => self.resolve_key(secrets).0,
        };
        Ok(match self.kind {
            ProviderKind::GoogleFree => Arc::new(GoogleFree::new(id, self.base_url.clone())),
            ProviderKind::Deepl => {
                let key = key.ok_or_else(|| ProviderError::Config(format!("{id}: missing API key")))?;
                Arc::new(DeepL::new(id, key, self.base_url.clone()))
            }
            ProviderKind::OpenaiCompatible => {
                let base_url = self
                    .base_url
                    .clone()
                    .filter(|u| !u.trim().is_empty())
                    .ok_or_else(|| ProviderError::Config(format!("{id}: missing base URL")))?;
                let model = self
                    .model
                    .clone()
                    .filter(|m| !m.trim().is_empty())
                    .ok_or_else(|| ProviderError::Config(format!("{id}: missing model")))?;
                Arc::new(OpenAiCompatible::new(id, self.name.clone(), base_url, key, model))
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

    /// Write atomically (temp file + rename) so a crash never leaves a half-written config.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Problems that would make the config unusable, as user-facing messages.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if !(1..=120).contains(&self.timeout_secs) {
            errors.push("Timeout must be between 1 and 120 seconds".into());
        }
        if self.default_target.is_empty() || self.default_target == "auto" {
            errors.push("Default target language must be a specific language".into());
        }
        let mut seen = std::collections::HashSet::new();
        for p in &self.providers {
            let valid_id = !p.id.is_empty()
                && p.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
            if !valid_id {
                errors.push(format!("Provider id \"{}\" may only use a-z, 0-9, - and _", p.id));
            }
            if !seen.insert(p.id.as_str()) {
                errors.push(format!("Provider id \"{}\" is used twice", p.id));
            }
            if p.kind == ProviderKind::OpenaiCompatible {
                if p.base_url.as_deref().is_none_or(|u| !(u.starts_with("http://") || u.starts_with("https://"))) {
                    errors.push(format!("{}: base URL must start with http:// or https://", p.id));
                }
                if p.model.as_deref().is_none_or(|m| m.trim().is_empty()) {
                    errors.push(format!("{}: model is required", p.id));
                }
            }
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;

    #[test]
    fn default_config_round_trips() {
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.providers.len(), 3);
        assert_eq!(back.providers[2].kind, ProviderKind::OpenaiCompatible);
        assert!(!back.providers[2].enabled);
    }

    #[test]
    fn configs_without_shortcuts_section_get_defaults() {
        let c: Config = toml::from_str("default_target = \"vi\"").unwrap();
        assert_eq!(c.shortcuts, Shortcuts::default());
        let c: Config = toml::from_str("[shortcuts]\nreplace_selection = \"\"").unwrap();
        assert_eq!(c.shortcuts.translate_selection, "Ctrl+Alt+E");
        assert!(c.shortcuts.replace_selection.is_empty());
    }

    #[test]
    fn deepl_without_key_is_a_config_error() {
        let p = ProviderConfig::new("deepl", ProviderKind::Deepl);
        assert!(matches!(p.build(&MemoryStore::default()), Err(ProviderError::Config(_))));
    }

    #[test]
    fn key_resolution_order() {
        let secrets = MemoryStore::default();
        let mut p = ProviderConfig::new("deepl", ProviderKind::Deepl);
        assert_eq!(p.resolve_key(&secrets).1, KeySource::None);
        secrets.set("deepl", "from-keychain").unwrap();
        assert_eq!(p.resolve_key(&secrets), (Some("from-keychain".into()), KeySource::Keychain));
        p.api_key = Some("from-config".into());
        assert_eq!(p.resolve_key(&secrets), (Some("from-config".into()), KeySource::Config));
        assert!(p.build(&secrets).is_ok());
    }

    #[test]
    fn validate_reports_problems() {
        assert!(Config::default().validate().is_empty());
        let mut c = Config { timeout_secs: 0, ..Config::default() };
        c.providers.push(ProviderConfig::new("google", ProviderKind::GoogleFree));
        c.providers.push(ProviderConfig::new("Bad Id", ProviderKind::OpenaiCompatible));
        let errors = c.validate().join("\n");
        assert!(errors.contains("Timeout"));
        assert!(errors.contains("used twice"));
        assert!(errors.contains("a-z, 0-9"));
        assert!(errors.contains("base URL"));
        assert!(errors.contains("model is required"));
    }

    #[test]
    fn save_then_load() {
        let dir = std::env::temp_dir().join(format!("corvus-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let c = Config { default_target: "vi".into(), ..Config::default() };
        c.save(&path).unwrap();
        assert_eq!(Config::load_or_init(&path).unwrap().default_target, "vi");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
