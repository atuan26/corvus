use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::AppState;

pub const ID: &str = "main";

fn accel(keys: &str) -> Option<&str> {
    (!keys.is_empty()).then_some(keys)
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let keys = app.state::<AppState>().config.shortcuts.clone();
    let show = MenuItem::with_id(app, "show", "Show Corvus", true, accel(&keys.show_window))?;
    let translate =
        MenuItem::with_id(app, "translate", "Translate selection", true, accel(&keys.translate_selection))?;
    let replace = MenuItem::with_id(app, "replace", "Replace selection", true, accel(&keys.replace_selection))?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &translate, &replace, &separator, &quit])?;
    let mut builder = TrayIconBuilder::with_id(ID)
        .tooltip("Corvus")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => crate::show_main_window(app),
            "translate" => crate::actions::translate_selection(app),
            "replace" => crate::actions::replace_selection(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                crate::show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
