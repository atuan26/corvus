use std::sync::LazyLock;

use corvus_core::dictionary::Wiktionary;
use corvus_core::lang::{self, Language};
use corvus_core::speech::GoogleSpeech;
use corvus_core::{Attempt, Ipa, ProviderInfo, RouterError, Shortcuts, TranslateRequest, Translation};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::popup::{self, Popup};
use crate::{AppState, Prefs};

/// Error shape the UI can render directly: a message plus per-provider details.
#[derive(Debug, Serialize)]
pub struct UiError {
    message: String,
    attempts: Vec<Attempt>,
}

impl From<RouterError> for UiError {
    fn from(e: RouterError) -> Self {
        match e {
            RouterError::AllFailed(attempts) => Self { message: "All providers failed".into(), attempts },
            other => Self { message: other.to_string(), attempts: Vec::new() },
        }
    }
}

#[tauri::command]
pub async fn translate(
    state: State<'_, AppState>,
    request: TranslateRequest,
    provider: Option<String>,
) -> Result<Translation, UiError> {
    Ok(state.router().translate(&request, provider.as_deref()).await?)
}

static WIKTIONARY: LazyLock<Wiktionary> = LazyLock::new(|| Wiktionary::new(None));
static SPEECH: LazyLock<GoogleSpeech> = LazyLock::new(|| GoogleSpeech::new(None));

/// IPA for a word or short phrase; empty when Wiktionary has none.
#[tauri::command]
pub async fn ipa(word: String, lang: String) -> Result<Vec<Ipa>, String> {
    WIKTIONARY.ipa(&word, &lang).await.map_err(|e| e.to_string())
}

/// Reads `text` aloud through the system's audio output.
#[tauri::command]
pub async fn speak(text: String, lang: String) -> Result<(), String> {
    let mp3 = SPEECH.speak(&text, &lang).await.map_err(|e| e.to_string())?;
    crate::audio::play(mp3).await
}

#[tauri::command]
pub fn providers(state: State<'_, AppState>) -> Vec<ProviderInfo> {
    state.router().providers()
}

#[tauri::command]
pub fn languages() -> &'static [Language] {
    lang::LANGUAGES
}

#[derive(Serialize)]
pub struct AppInfo {
    default_source: String,
    default_target: String,
    config_path: Option<String>,
    shortcuts: Shortcuts,
    warnings: Vec<String>,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    let config = state.config();
    AppInfo {
        default_source: config.default_source,
        default_target: config.default_target,
        config_path: state.config_path.as_ref().map(|p| p.display().to_string()),
        shortcuts: config.shortcuts,
        warnings: state.warnings(),
    }
}

#[tauri::command]
pub fn get_prefs(state: State<'_, AppState>) -> Prefs {
    state.prefs.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_prefs(state: State<'_, AppState>, prefs: Prefs) {
    *state.prefs.lock().unwrap() = prefs;
}

/// Non-destructive, so a remounted popup (e.g. React StrictMode in dev) still gets the text.
#[tauri::command]
pub fn popup_text(window: tauri::Window, state: State<'_, AppState>) -> Option<String> {
    state.popup_text.lock().unwrap().get(window.label()).cloned()
}

/// Move a popup's text into the main window for editing (`view` picks Translate or Ask).
#[tauri::command]
pub fn open_in_main(app: AppHandle, window: tauri::Window, text: String, view: Option<String>) -> Result<(), String> {
    if let Some(p) = Popup::from_label(window.label()) {
        popup::hide(&app, p);
    }
    crate::show_main_window(&app);
    let event = if view.as_deref() == Some("ask") { "load-ask" } else { "load-text" };
    app.emit_to("main", event, text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn hide_popup(app: AppHandle, window: tauri::Window) {
    if let Some(p) = Popup::from_label(window.label()) {
        popup::hide(&app, p);
    }
}

/// Hide the popup, let focus return to the previous app, then paste `text` over its selection.
#[tauri::command]
pub async fn paste_text(app: AppHandle, window: tauri::Window, text: String) -> Result<(), String> {
    if let Some(p) = Popup::from_label(window.label()) {
        popup::hide(&app, p);
    }
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || app2.state::<crate::selection::ClipboardState>().paste(&text))
        .await
        .map_err(|e| e.to_string())?
}
