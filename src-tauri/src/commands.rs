use corvus_core::lang::{self, Language};
use corvus_core::{Attempt, ProviderInfo, RouterError, TranslateRequest, Translation};
use serde::Serialize;
use tauri::State;

use crate::AppState;

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
    Ok(state.router.translate(&request, provider.as_deref()).await?)
}

#[tauri::command]
pub fn providers(state: State<'_, AppState>) -> Vec<ProviderInfo> {
    state.router.providers()
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
    warnings: Vec<String>,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        default_source: state.config.default_source.clone(),
        default_target: state.config.default_target.clone(),
        config_path: state.config_path.as_ref().map(|p| p.display().to_string()),
        warnings: state.warnings.clone(),
    }
}
