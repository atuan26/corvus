//! Small always-on-top window that shows the translation of the current selection.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::AppState;

pub const LABEL: &str = "popup";
const WIDTH: f64 = 440.0;
const HEIGHT: f64 = 260.0;

fn window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("popup.html".into()))
        .title("Corvus translation")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()?;
    // Tiling window managers (i3, sway, bspwm) tile "normal" windows; a dialog hint makes them
    // float it at the position we choose. Applied while the window is still unmapped.
    #[cfg(target_os = "linux")]
    {
        let w2 = w.clone();
        w.run_on_main_thread(move || {
            use gtk::prelude::{GtkWindowExt, WidgetExt};
            if let Ok(gtk_window) = w2.gtk_window() {
                gtk_window.set_type_hint(gtk::gdk::WindowTypeHint::Dialog);
                // The GtkWindow is already realized, so also set it on the GdkWindow, which
                // writes _NET_WM_WINDOW_TYPE for the next map.
                if let Some(gdk_window) = gtk_window.window() {
                    gdk_window.set_type_hint(gtk::gdk::WindowTypeHint::Dialog);
                }
            }
        })?;
    }
    Ok(w)
}

/// Window managers may briefly move focus while mapping the popup; ignore blur right after showing.
pub fn recently_shown(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .popup_shown_at
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
}

/// Place the popup just below-right of the cursor, kept inside the cursor's monitor.
/// On Wayland the compositor ignores client positioning and places the window itself.
fn position_near_cursor(app: &AppHandle, w: &WebviewWindow) {
    let Ok(cursor) = app.cursor_position() else { return };
    let scale = w.scale_factor().unwrap_or(1.0);
    let (pw, ph) = (WIDTH * scale, HEIGHT * scale);
    let mut x = cursor.x + 12.0 * scale;
    let mut y = cursor.y + 16.0 * scale;
    if let Ok(Some(m)) = app.monitor_from_point(cursor.x, cursor.y) {
        let area = m.work_area();
        let (left, top) = (area.position.x as f64, area.position.y as f64);
        let (right, bottom) = (left + area.size.width as f64, top + area.size.height as f64);
        if x + pw > right {
            x = (cursor.x - pw - 12.0 * scale).max(left);
        }
        if y + ph > bottom {
            y = (cursor.y - ph - 16.0 * scale).max(top);
        }
    }
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

pub fn show_text(app: &AppHandle, text: String) -> tauri::Result<()> {
    *app.state::<AppState>().popup_text.lock().unwrap() = Some(text.clone());
    let w = window(app)?;
    position_near_cursor(app, &w);
    *app.state::<AppState>().popup_shown_at.lock().unwrap() = Some(Instant::now());
    w.show()?;
    w.set_focus()?;
    app.emit_to(LABEL, "popup-text", text)?;
    Ok(())
}
