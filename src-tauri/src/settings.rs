//! Settings screen backend: read, validate, save and apply configuration without a restart.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use corvus_core::{
    AssistantConfig, Config, KeySource, Keychain, ProviderConfig, Router, SecretStore, Shortcuts, TranslateRequest,
    Translation,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::Shortcut;

use crate::{shortcuts, tray, AppState};

/// Keychain calls may block on D-Bus or prompt to unlock a wallet; keep them off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

pub fn open(app: &AppHandle) {
    crate::show_main_window(app);
    let _ = app.emit_to("main", "open-settings", ());
}

/// Replace provider and/or shortcut warnings, then tell windows to refresh.
pub fn set_warnings(app: &AppHandle, providers: Option<Vec<String>>, shortcuts: Option<Vec<String>>) {
    {
        let state = app.state::<AppState>();
        let mut w = state.warnings.lock().unwrap();
        if let Some(p) = providers {
            w.providers = p;
        }
        if let Some(s) = shortcuts {
            w.shortcuts = s;
        }
    }
    let _ = app.emit("settings-changed", ());
}

#[derive(Serialize)]
pub struct ProviderView {
    #[serde(flatten)]
    config: ProviderConfig,
    key_source: KeySource,
    requires_key: bool,
}

#[derive(Serialize)]
pub struct AssistantView {
    #[serde(flatten)]
    config: AssistantConfig,
    key_source: KeySource,
}

#[derive(Serialize)]
pub struct SettingsView {
    default_source: String,
    default_target: String,
    timeout_secs: u64,
    shortcuts: Shortcuts,
    ocr: corvus_core::ocr::OcrConfig,
    providers: Vec<ProviderView>,
    assistants: Vec<AssistantView>,
    /// Where new secrets go when the keychain is down: "keychain" or "file".
    secrets_location: &'static str,
    config_path: Option<String>,
    /// `None` when the keychain works, otherwise why it doesn't.
    keychain_error: Option<String>,
}

#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
    ask: State<'_, crate::ask::AskState>,
) -> Result<SettingsView, String> {
    let config = state.config();
    let config_path = state.config_path.as_ref().map(|p| p.display().to_string());
    let ask_secrets = ask.secrets.clone();
    blocking(move || {
        let keychain_error = Keychain::probe().err();
        let secrets: &dyn SecretStore = if keychain_error.is_some() { &NoKeychain } else { &Keychain };
        let providers = config
            .providers
            .iter()
            .map(|p| {
                let key_source = p.resolve_key(secrets).1;
                // The UI never sees key material, only where it comes from.
                let config = ProviderConfig { api_key: None, ..p.clone() };
                ProviderView { config, key_source, requires_key: p.kind.requires_key() }
            })
            .collect();
        let assistants = config
            .assistants
            .iter()
            .map(|a| AssistantView {
                key_source: a.resolve_key(ask_secrets.as_ref()).1,
                config: AssistantConfig { api_key: None, ..a.clone() },
            })
            .collect();
        SettingsView {
            assistants,
            secrets_location: ask_secrets.location(),
            default_source: config.default_source,
            default_target: config.default_target,
            timeout_secs: config.timeout_secs,
            shortcuts: config.shortcuts,
            ocr: config.ocr,
            providers,
            config_path,
            keychain_error,
        }
    })
    .await
}

/// Skips the keychain when it's known to be down, so each lookup doesn't wait on a D-Bus timeout.
struct NoKeychain;

impl SecretStore for NoKeychain {
    fn get(&self, _: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn set(&self, _: &str, _: &str) -> Result<(), String> {
        Err("keychain unavailable".into())
    }
    fn delete(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct SettingsInput {
    default_source: String,
    default_target: String,
    timeout_secs: u64,
    shortcuts: Shortcuts,
    providers: Vec<ProviderConfig>,
    assistants: Vec<AssistantConfig>,
    ocr: corvus_core::ocr::OcrConfig,
}

/// Rebuild the provider chain from `config`, swap it in, and re-register shortcuts.
async fn apply(app: &AppHandle, config: Config) -> Result<Vec<String>, String> {
    let for_router = config.clone();
    let (router, provider_warnings) = blocking(move || Router::from_config(&for_router, &Keychain)).await?;
    let state = app.state::<AppState>();
    *state.router.write().unwrap() = Arc::new(router);
    let keys = config.shortcuts.clone();
    *state.config.write().unwrap() = config;
    let shortcut_warnings = shortcuts::apply(app, &keys);
    tray::refresh(app);
    let warnings = provider_warnings.iter().chain(&shortcut_warnings).cloned().collect();
    set_warnings(app, Some(provider_warnings), Some(shortcut_warnings));
    Ok(warnings)
}

fn write_config(state: &AppState, config: &Config) -> Result<(), String> {
    let path = state.config_path.as_ref().ok_or("No config directory on this system")?;
    config.save(path)
}

/// Validate and persist. On failure returns every problem so the UI can show them together.
#[tauri::command]
pub async fn save_settings(app: AppHandle, input: SettingsInput) -> Result<Vec<String>, Vec<String>> {
    let state = app.state::<AppState>();
    let old = state.config();
    let providers = input
        .providers
        .into_iter()
        .map(|mut p| {
            // Keys aren't round-tripped through the UI; keep any fallback key from the file.
            p.api_key = old.providers.iter().find(|o| o.id == p.id).and_then(|o| o.api_key.clone());
            p
        })
        .collect();
    let assistants = input
        .assistants
        .into_iter()
        .map(|mut a| {
            a.api_key = old.assistants.iter().find(|o| o.id == a.id).and_then(|o| o.api_key.clone());
            a
        })
        .collect();
    let config = Config {
        assistants,
        ocr: input.ocr,
        default_source: input.default_source,
        default_target: input.default_target,
        timeout_secs: input.timeout_secs,
        shortcuts: input.shortcuts,
        providers,
    };
    let mut errors = config.validate();
    errors.extend(shortcuts::validate(&config.shortcuts));
    if !errors.is_empty() {
        return Err(errors);
    }
    write_config(&state, &config).map_err(|e| vec![e])?;
    apply(&app, config).await.map_err(|e| vec![e])
}

#[derive(Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum KeyStore {
    Keychain,
    /// Plain text in config.toml, for systems without a keychain.
    Config,
}

/// Store (or with `key: None`, remove) a provider's key, then rebuild providers.
#[tauri::command]
pub async fn set_api_key(
    app: AppHandle,
    provider_id: String,
    key: Option<String>,
    store: KeyStore,
) -> Result<KeySource, String> {
    let state = app.state::<AppState>();
    let mut config = state.config();
    let key = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    let index = config.providers.iter().position(|p| p.id == provider_id);

    if store == KeyStore::Keychain {
        let (id, k) = (provider_id.clone(), key.clone());
        blocking(move || match k {
            Some(k) => Keychain.set(&id, &k),
            None => Keychain.delete(&id),
        })
        .await??;
    }
    if let Some(i) = index {
        let p = &mut config.providers[i];
        let new_literal = if store == KeyStore::Config { key.clone() } else { None };
        // Moving a key to the keychain (or removing it) also clears any plain-text copy.
        if p.api_key != new_literal {
            p.api_key = new_literal;
            write_config(&state, &config)?;
        }
    } else if store == KeyStore::Config {
        return Err("Save the new provider first, then add its key".into());
    }

    apply(&app, config.clone()).await?;
    let provider = config.providers.into_iter().find(|p| p.id == provider_id);
    Ok(match provider {
        Some(p) => blocking(move || p.resolve_key(&Keychain).1).await?,
        None if key.is_some() => KeySource::Keychain,
        None => KeySource::None,
    })
}

/// Translate a fixed phrase with an unsaved provider definition, so users can check a URL,
/// model or key before saving.
#[tauri::command]
pub async fn test_provider(state: State<'_, AppState>, provider: ProviderConfig) -> Result<Translation, String> {
    let config = state.config();
    // Prefer the key already saved for this id (the form never holds it).
    let saved_key = config.providers.iter().find(|p| p.id == provider.id).and_then(|p| p.api_key.clone());
    let provider = ProviderConfig { api_key: saved_key, ..provider };
    let translator = blocking(move || provider.build(&Keychain)).await?.map_err(|e| e.to_string())?;
    let target = if config.default_target == "en" { "es".to_string() } else { config.default_target };
    let req = TranslateRequest::new("Hello, world!", "en", target);
    let timeout = Duration::from_secs(config.timeout_secs.max(1));
    match tokio::time::timeout(timeout, translator.translate(&req)).await {
        Ok(r) => r.map_err(|e| e.to_string()),
        Err(_) => Err(format!("No answer within {} seconds", timeout.as_secs())),
    }
}

/// Check one key sequence as the recorder produces it.
#[tauri::command]
pub fn validate_shortcut(keys: String) -> Result<(), String> {
    Shortcut::from_str(&keys).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pause_shortcuts(app: AppHandle) {
    shortcuts::pause(&app);
}

#[tauri::command]
pub fn resume_shortcuts(app: AppHandle) {
    let keys = app.state::<AppState>().config().shortcuts;
    let warnings = shortcuts::apply(&app, &keys);
    set_warnings(&app, None, Some(warnings));
}
