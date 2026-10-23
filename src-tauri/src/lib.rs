mod actions;
mod ask;
mod commands;
mod popup;
mod selection;
mod settings;
mod shortcuts;
mod tray;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use corvus_core::{Config, Keychain, Router};
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
    /// Swapped wholesale when settings are saved; readers clone the Arc and drop the lock.
    pub router: RwLock<Arc<Router>>,
    pub config: RwLock<Config>,
    pub config_path: Option<PathBuf>,
    /// Problem reading the config file at startup.
    pub load_error: Option<String>,
    pub warnings: Mutex<Warnings>,
    pub prefs: Mutex<Prefs>,
    /// Text for the popup; read on mount because a new window misses events emitted before its
    /// listener exists.
    pub popup_text: Mutex<std::collections::HashMap<String, String>>,
    pub popup_shown_at: Mutex<Option<std::time::Instant>>,
}

/// Shown in the main window. Kept per source so each can be refreshed independently.
#[derive(Default)]
pub struct Warnings {
    pub providers: Vec<String>,
    pub shortcuts: Vec<String>,
}

impl AppState {
    pub fn warnings(&self) -> Vec<String> {
        let w = self.warnings.lock().unwrap();
        self.load_error.iter().chain(&w.providers).chain(&w.shortcuts).cloned().collect()
    }

    pub fn router(&self) -> Arc<Router> {
        self.router.read().unwrap().clone()
    }

    pub fn config(&self) -> Config {
        self.config.read().unwrap().clone()
    }
}

fn load_state() -> AppState {
    let config_path = Config::default_path();
    let (config, load_error) = match config_path.as_deref().map(Config::load_or_init) {
        Some(Ok(c)) => (c, None),
        Some(Err(e)) => (Config::default(), Some(format!("Using defaults, config could not be read: {e}"))),
        None => (Config::default(), None),
    };
    let (router, provider_warnings) = Router::from_config(&config, &Keychain);
    let prefs = Prefs { source: config.default_source.clone(), target: config.default_target.clone(), provider: None };
    AppState {
        router: RwLock::new(Arc::new(router)),
        config: RwLock::new(config),
        config_path,
        load_error,
        warnings: Mutex::new(Warnings { providers: provider_warnings, shortcuts: Vec::new() }),
        prefs: Mutex::new(prefs),
        popup_text: Mutex::new(std::collections::HashMap::new()),
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
        Some("--ask-selection") => actions::ask_selection(app),
        Some("--hidden") => {}
        Some(other) => tracing::warn!("unknown argument: {other}"),
        None if first_launch => {}
        None => show_main_window(app),
    }
}

pub fn run() {
    let state = load_state();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| handle_args(app, &args, false)))
        .manage(state)
        .manage(selection::ClipboardState::new())
        .manage(shortcuts::Bindings::default())
        .manage(ask::AskState::new())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            tray::create(app.handle())?;
            let shortcut_warnings = match shortcuts::init(app) {
                Ok(()) => {
                    let keys = app.state::<AppState>().config().shortcuts;
                    shortcuts::apply(app.handle(), &keys)
                }
                Err(e) => vec![format!("Global shortcuts are unavailable: {e}")],
            };
            settings::set_warnings(app.handle(), None, Some(shortcut_warnings));
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
                if popup::Popup::from_label(window.label()).is_some() && !popup::recently_shown(window.app_handle()) =>
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
            settings::get_settings,
            settings::save_settings,
            settings::set_api_key,
            settings::test_provider,
            settings::validate_shortcut,
            settings::pause_shortcuts,
            settings::resume_shortcuts,
            commands::paste_text,
            ask::list_assistants,
            ask::ask_start,
            ask::ask_cancel,
            ask::chatgpt_status,
            ask::chatgpt_sign_in,
            ask::chatgpt_device_start,
            ask::chatgpt_device_finish,
            ask::chatgpt_cancel_sign_in,
            ask::chatgpt_sign_out,
            ask::chatgpt_models,
            ask::set_assistant_key,
            ask::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Corvus");
}
