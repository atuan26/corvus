mod actions;
mod commands;
mod popup;
mod selection;
mod shortcuts;
mod tray;

use std::path::PathBuf;
use std::sync::Mutex;

use corvus_core::{Config, Router};
use serde::{Deserialize, Serialize};
use tauri::Manager;

/// Languages and provider last chosen in the UI. The main window owns them; the popup and
/// replace-selection reuse them so every entry point translates the same way.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prefs {
    pub source: String,
    pub target: String,
    pub provider: Option<String>,
}

pub struct AppState {
    pub router: Router,
    pub config: Config,
    pub config_path: Option<PathBuf>,
    pub warnings: Mutex<Vec<String>>,
    pub prefs: Mutex<Prefs>,
    /// Text waiting for the popup; it is fetched on mount because a new window misses events
    /// emitted before its listener exists.
    pub popup_text: Mutex<Option<String>>,
    pub popup_shown_at: Mutex<Option<std::time::Instant>>,
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
    let prefs = Prefs {
        source: config.default_source.clone(),
        target: config.default_target.clone(),
        provider: None,
    };
    AppState {
        router,
        config,
        config_path,
        warnings: Mutex::new(warnings),
        prefs: Mutex::new(prefs),
        popup_text: Mutex::new(None),
        popup_shown_at: Mutex::new(None),
    }
}

pub fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Command-line actions, so desktops without global hotkey support (Wayland) can bind
/// `corvus-app --translate-selection` in their own shortcut settings. A second launch forwards
/// its arguments to the running instance.
fn handle_args(app: &tauri::AppHandle, args: &[String], first_launch: bool) {
    match args.iter().skip(1).map(String::as_str).find(|a| a.starts_with("--")) {
        Some("--translate-selection") => actions::translate_selection(app),
        Some("--replace-selection") => actions::replace_selection(app),
        Some("--hidden") => {}
        Some(other) => tracing::warn!("unknown argument: {other}"),
        None if first_launch => {}
        None => show_main_window(app),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| handle_args(app, &args, false)))
        .manage(load_state())
        .manage(selection::ClipboardState::new())
        .setup(|app| {
            tray::create(app.handle())?;
            shortcuts::register(app);
            let args: Vec<String> = std::env::args().collect();
            // The main window starts hidden; launches for an action or `--hidden` keep it that way.
            if !args.iter().skip(1).any(|a| a.starts_with("--")) {
                show_main_window(app.handle());
            }
            handle_args(app.handle(), &args, true);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the main window hides it to the tray; quit from the tray menu.
            tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                if window.app_handle().tray_by_id(tray::ID).is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            tauri::WindowEvent::Focused(false)
                if window.label() == popup::LABEL && !popup::recently_shown(window.app_handle()) =>
            {
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate,
            commands::providers,
            commands::languages,
            commands::app_info,
            commands::get_prefs,
            commands::set_prefs,
            commands::popup_text,
            commands::open_in_main,
            commands::hide_popup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Corvus");
}
