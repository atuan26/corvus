use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;

use corvus_core::Shortcuts;
use tauri::{App, AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[derive(Clone, Copy)]
enum Action {
    ShowWindow,
    TranslateSelection,
    ReplaceSelection,
}

impl Action {
    fn run(self, app: &AppHandle) {
        match self {
            Action::ShowWindow => crate::show_main_window(app),
            Action::TranslateSelection => crate::actions::translate_selection(app),
            Action::ReplaceSelection => crate::actions::replace_selection(app),
        }
    }
}

/// Registered shortcut id → action. Replaced on every `apply`.
#[derive(Default)]
pub struct Bindings(Mutex<HashMap<u32, Action>>);

/// Install the plugin once; `apply` (re)registers keys.
pub fn init(app: &App) -> tauri::Result<()> {
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let action = app.state::<Bindings>().0.lock().unwrap().get(&shortcut.id()).copied();
            if let Some(action) = action {
                action.run(app);
            }
        })
        .build();
    app.handle().plugin(plugin)
}

fn entries(keys: &Shortcuts) -> [(&str, &String, Action); 3] {
    [
        ("Show Corvus", &keys.show_window, Action::ShowWindow),
        ("Translate selection", &keys.translate_selection, Action::TranslateSelection),
        ("Replace selection", &keys.replace_selection, Action::ReplaceSelection),
    ]
}

/// Parse errors and duplicates, as user-facing messages. Empty means valid.
pub fn validate(keys: &Shortcuts) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen: HashMap<u32, &str> = HashMap::new();
    for (label, keys, _) in entries(keys) {
        if keys.trim().is_empty() {
            continue;
        }
        match Shortcut::from_str(keys) {
            Ok(s) => {
                if let Some(other) = seen.insert(s.id(), label) {
                    errors.push(format!("{label} and {other} both use {keys}"));
                }
            }
            Err(e) => errors.push(format!("{label}: \"{keys}\" is not a valid shortcut ({e})")),
        }
    }
    errors
}

/// Replace all registrations with `keys`. Returns warnings for keys another app holds.
/// On Wayland the compositor owns global keys, so failures are expected and non-fatal
/// (upstream crow-translate #635 crashed here).
pub fn apply(app: &AppHandle, keys: &Shortcuts) -> Vec<String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut bindings = HashMap::new();
    let mut warnings = Vec::new();
    for (label, keys, action) in entries(keys) {
        if keys.trim().is_empty() {
            continue;
        }
        let Ok(s) = Shortcut::from_str(keys) else {
            warnings.push(format!("{label}: \"{keys}\" is not a valid shortcut"));
            continue;
        };
        match gs.register(s) {
            Ok(()) => {
                bindings.insert(s.id(), action);
            }
            Err(e) => {
                tracing::debug!("register {keys}: {e}");
                warnings.push(format!(
                    "{keys} is already used by another app. Choose another key for \"{label}\" in Settings."
                ));
            }
        }
    }
    *app.state::<Bindings>().0.lock().unwrap() = bindings;
    warnings
}

/// Release all keys, e.g. while the settings screen records a new shortcut.
pub fn pause(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
    app.state::<Bindings>().0.lock().unwrap().clear();
}
