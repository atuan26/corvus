mod commands;
mod shortcuts;
mod tray;

use std::path::PathBuf;

use corvus_core::{Config, Router};
use tauri::Manager;

pub struct AppState {
    pub router: Router,
    pub config: Config,
    pub config_path: Option<PathBuf>,
    pub warnings: Vec<String>,
}

fn load_state() -> AppState {
    let config_path = Config::default_path();
    let mut warnings = Vec::new();
    let config = match config_path.as_deref().map(Config::load_or_init) {
        Some(Ok(c)) => c,
        Some(Err(e)) => {
            warnings.push(format!("Using defaults, config could not be read: {e}"));
            Config::default()
        }
        None => Config::default(),
    };
    let (router, provider_warnings) = Router::from_config(&config);
    warnings.extend(provider_warnings);
    AppState { router, config, config_path, warnings }
}

pub fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main_window(app)))
        .manage(load_state())
        .setup(|app| {
            tray::create(app.handle())?;
            shortcuts::register(app);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window hides it to the tray; quit from the tray menu.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.app_handle().tray_by_id(tray::ID).is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate,
            commands::providers,
            commands::languages,
            commands::app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Corvus");
}
