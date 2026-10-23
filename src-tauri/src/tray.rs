use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::AppState;

pub const ID: &str = "main";

fn accel(keys: &str) -> Option<&str> {
    (!keys.is_empty()).then_some(keys)
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let keys = app.state::<AppState>().config().shortcuts;
    let show = MenuItem::with_id(app, "show", "Show Corvus", true, accel(&keys.show_window))?;
    let translate =
        MenuItem::with_id(app, "translate", "Translate selection", true, accel(&keys.translate_selection))?;
    let replace = MenuItem::with_id(app, "replace", "Replace selection", true, accel(&keys.replace_selection))?;
    let ask = MenuItem::with_id(app, "ask", "Ask AI about selection", true, accel(&keys.ask_selection))?;
    let ocr = MenuItem::with_id(app, "ocr", "Translate screen area", true, accel(&keys.ocr_translate))?;
    let ocr_copy = MenuItem::with_id(app, "ocr-copy", "Copy text from screen area", true, accel(&keys.ocr_copy))?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    Menu::with_items(app, &[&show, &translate, &replace, &ask, &ocr, &ocr_copy, &separator, &settings, &quit])
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id(ID)
        .tooltip("Corvus")
        .menu(&menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => crate::show_main_window(app),
            "translate" => crate::actions::translate_selection(app),
            "replace" => crate::actions::replace_selection(app),
            "ask" => crate::actions::ask_selection(app),
            "ocr" => crate::ocr::start(app, crate::ocr::Mode::Translate),
            "ocr-copy" => crate::ocr::start(app, crate::ocr::Mode::Copy),
            "settings" => crate::settings::open(app),
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

/// Rebuild the menu so accelerator hints match the current shortcuts.
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(ID), menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}
