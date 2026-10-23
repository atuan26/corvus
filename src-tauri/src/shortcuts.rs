use tauri::App;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Register global shortcuts. Failure is non-fatal: on Wayland the compositor owns global
/// keys, so we log and carry on instead of crashing (upstream crow-translate #635).
pub fn register(app: &App) {
    let show = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyC);
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, shortcut, event| {
            if event.state() == ShortcutState::Pressed && shortcut == &show {
                crate::show_main_window(app);
            }
        })
        .build();
    if let Err(e) = app.handle().plugin(plugin) {
        tracing::warn!("global shortcuts unavailable: {e}");
        return;
    }
    if let Err(e) = app.global_shortcut().register(show) {
        tracing::warn!("could not register Ctrl+Alt+C: {e}");
    }
}
