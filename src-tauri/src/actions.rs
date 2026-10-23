//! Actions triggered from global shortcuts and the tray.

use corvus_core::TranslateRequest;
use tauri::{AppHandle, Manager};

use crate::popup::Popup;
use crate::selection::ClipboardState;
use crate::AppState;

async fn selected_text(app: &AppHandle) -> Result<String, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || app.state::<ClipboardState>().read_selection())
        .await
        .map_err(|e| e.to_string())?
}

pub fn translate_selection(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let text = selected_text(&app).await.unwrap_or_else(|e| {
            tracing::info!("translate selection: {e}");
            String::new()
        });
        // An empty popup explains that nothing was selected and offers manual input.
        if let Err(e) = crate::popup::show_text(&app, Popup::Translate, text) {
            tracing::warn!("cannot show popup: {e}");
        }
    });
}

pub fn ask_selection(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Without a selection the popup offers a free-form question instead.
        let text = selected_text(&app).await.unwrap_or_default();
        if let Err(e) = crate::popup::show_text(&app, Popup::Ask, text) {
            tracing::warn!("cannot show ask popup: {e}");
        }
    });
}

pub fn replace_selection(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let text = match selected_text(&app).await {
            Ok(t) => t,
            Err(e) => return tracing::info!("replace selection: {e}"),
        };
        let state = app.state::<AppState>();
        let prefs = state.prefs.lock().unwrap().clone();
        let req = TranslateRequest::new(text.clone(), prefs.source, prefs.target);
        let router = state.router();
        match router.translate(&req, prefs.provider.as_deref()).await {
            Ok(t) => {
                let app2 = app.clone();
                let pasted = tauri::async_runtime::spawn_blocking(move || {
                    app2.state::<ClipboardState>().paste(&t.text)
                })
                .await;
                match pasted {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => tracing::warn!("paste failed: {e}"),
                    Err(e) => tracing::warn!("paste task failed: {e}"),
                }
            }
            Err(e) => {
                tracing::warn!("replace selection: {e}");
                // Show what went wrong instead of failing silently.
                let _ = crate::popup::show_text(&app, Popup::Translate, text);
            }
        }
    });
}
