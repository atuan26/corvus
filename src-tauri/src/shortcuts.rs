use std::collections::HashMap;
use std::str::FromStr;

use tauri::{App, AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::AppState;

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

/// Register the configured global shortcuts. Failures become UI warnings rather than errors:
/// on Wayland the compositor owns global keys, so we must not crash (upstream crow-translate #635).
pub fn register(app: &App) {
    let state = app.state::<AppState>();
    let cfg = state.config.shortcuts.clone();
    let mut warnings = Vec::new();
    let mut bindings: HashMap<u32, Action> = HashMap::new();
    let mut shortcuts = Vec::new();
    for (keys, action) in [
        (&cfg.show_window, Action::ShowWindow),
        (&cfg.translate_selection, Action::TranslateSelection),
        (&cfg.replace_selection, Action::ReplaceSelection),
    ] {
        if keys.trim().is_empty() {
            continue;
        }
        match Shortcut::from_str(keys) {
            Ok(s) => {
                bindings.insert(s.id(), action);
                shortcuts.push((keys.clone(), s));
            }
            Err(e) => warnings.push(format!("Invalid shortcut \"{keys}\": {e}")),
        }
    }

    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some(action) = bindings.get(&shortcut.id()) {
                    action.run(app);
                }
            }
        })
        .build();
    if let Err(e) = app.handle().plugin(plugin) {
        warnings.push(format!("Global shortcuts are unavailable: {e}"));
    } else {
        for (keys, s) in shortcuts {
            if let Err(e) = app.global_shortcut().register(s) {
                tracing::debug!("register {keys}: {e}");
                warnings.push(format!(
                    "{keys} is already used by another app. Pick another key under [shortcuts] in the config file."
                ));
            }
        }
    }
    for w in &warnings {
        tracing::warn!("{w}");
    }
    state.warnings.lock().unwrap().extend(warnings);
}
