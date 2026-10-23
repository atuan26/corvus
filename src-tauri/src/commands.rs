use corvus_core::lang::{self, Language};
use corvus_core::{Attempt, ProviderInfo, RouterError, Shortcuts, TranslateRequest, Translation};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{popup, AppState, Prefs};

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
pub fn popup_text(state: State<'_, AppState>) -> Option<String> {
    state.popup_text.lock().unwrap().clone()
}

/// Move the popup's text into the main window for editing.
#[tauri::command]
pub fn open_in_main(app: AppHandle, text: String) -> Result<(), String> {
    hide_popup(app.clone());
    crate::show_main_window(&app);
    app.emit_to("main", "load-text", text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn hide_popup(app: AppHandle) {
    if let Some(w) = app.get_webview_window(popup::LABEL) {
        let _ = w.hide();
    }
}
