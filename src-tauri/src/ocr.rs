//! Screen-area OCR: capture the monitor under the cursor, let the user pick an area in a
//! full-screen overlay, recognize its text locally, then translate or copy it.

use std::io::Cursor;
use std::sync::Mutex;

use corvus_core::ocr::join_lines;
use image::{imageops, DynamicImage, GrayImage, RgbaImage};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use crate::capture::{self, Area};
use crate::popup::{self, Popup};
use crate::selection::ClipboardState;
use crate::AppState;

pub const LABEL: &str = "snip";

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Translate,
    Copy,
}

struct Snip {
    image: RgbaImage,
    png: Vec<u8>,
    mode: Mode,
}

#[derive(Default)]
pub struct SnipState(Mutex<Option<Snip>>);

fn encode_png(img: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

pub fn start(app: &AppHandle, mode: Mode) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = begin(&app, mode).await {
            tracing::warn!("screen OCR: {e}");
            show_result(&app, Mode::Translate, String::new(), Some(e));
        }
    });
}

async fn begin(app: &AppHandle, mode: Mode) -> Result<(), String> {
    // Keep our own popups out of the picture.
    for p in Popup::ALL {
        popup::hide(app, p);
    }
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let cursor = app.cursor_position().map_err(|e| e.to_string())?;
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(|e| e.to_string())?
        .or(app.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("no monitor found")?;
    let area = Area {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
    };
    let image = capture::capture(area).await?;
    let png = tauri::async_runtime::spawn_blocking({
        let img = DynamicImage::ImageRgba8(image.clone());
        move || encode_png(&img)
    })
    .await
    .map_err(|e| e.to_string())??;
    *app.state::<SnipState>().0.lock().unwrap() = Some(Snip { image, png, mode });

    let window = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("snip.html".into()))
            .title("Corvus screen area")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?,
    };
    let _ = window.set_fullscreen(false);
    let _ = window.set_position(PhysicalPosition::new(area.x, area.y));
    let _ = window.set_size(PhysicalSize::new(area.width, area.height));
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_fullscreen(true);
    let _ = window.set_focus();
    let _ = app.emit_to(LABEL, "snip-ready", ());
    Ok(())
}

/// The captured screen as PNG, for the overlay's background.
#[tauri::command]
pub fn snip_image(state: tauri::State<'_, SnipState>) -> Result<tauri::ipc::Response, String> {
    let guard = state.0.lock().unwrap();
    let snip = guard.as_ref().ok_or("no screen capture in progress")?;
    Ok(tauri::ipc::Response::new(snip.png.clone()))
}

fn hide_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_fullscreen(false);
        let _ = w.hide();
    }
}

#[tauri::command]
pub fn snip_cancel(app: AppHandle) {
    app.state::<SnipState>().0.lock().unwrap().take();
    hide_overlay(&app);
}

/// Selected area in captured-image pixels.
#[derive(Deserialize)]
pub struct Rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Grayscale, dark-mode inversion and upscaling: Tesseract reads dark text on a light
/// background best, at roughly 30+ px letter height.
fn prepare(img: &RgbaImage) -> GrayImage {
    let mut gray = DynamicImage::ImageRgba8(img.clone()).to_luma8();
    let mean = gray.pixels().map(|p| p.0[0] as u64).sum::<u64>() / (gray.len().max(1) as u64);
    if mean < 110 {
        imageops::invert(&mut gray);
    }
    let factor = if gray.height() < 300 { 3 } else if gray.height() < 800 { 2 } else { 1 };
    if factor > 1 {
        gray = imageops::resize(&gray, gray.width() * factor, gray.height() * factor, imageops::FilterType::CatmullRom);
    }
    gray
}

#[tauri::command]
pub async fn snip_finish(app: AppHandle, rect: Rect) -> Result<(), String> {
    let snip = app.state::<SnipState>().0.lock().unwrap().take().ok_or("no screen capture in progress")?;
    hide_overlay(&app);
    let (w, h) = (snip.image.width(), snip.image.height());
    let x = rect.x.min(w.saturating_sub(1));
    let y = rect.y.min(h.saturating_sub(1));
    let crop = imageops::crop_imm(&snip.image, x, y, rect.width.clamp(1, w - x), rect.height.clamp(1, h - y)).to_image();
    let ocr = app.state::<AppState>().config().ocr;
    let mode = snip.mode;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let png = encode_png(&DynamicImage::ImageLuma8(prepare(&crop)))?;
        ocr.recognize(&png)
    })
    .await
    .map_err(|e| e.to_string())?;
    match result {
        Ok(text) => show_result(&app, mode, text, None),
        Err(e) => show_result(&app, mode, String::new(), Some(e)),
    }
    Ok(())
}

fn show_result(app: &AppHandle, mode: Mode, text: String, error: Option<String>) {
    let note = error.or_else(|| text.trim().is_empty().then(|| "No text found in that area".to_string()));
    match mode {
        // The popup translates on its own; give it whole sentences rather than OCR's line breaks.
        Mode::Translate => {
            let _ = popup::show_text(app, Popup::Translate, join_lines(&text));
            if let Some(n) = note {
                let _ = app.emit_to(Popup::Translate.label(), "popup-note", n);
            }
        }
        // Copy keeps the original lines (tables, lists) and opens the text for editing.
        Mode::Copy => {
            if !text.is_empty() {
                let _ = app.state::<ClipboardState>().copy(&text);
            }
            crate::show_main_window(app);
            let _ = app.emit_to("main", "load-text", text);
            if let Some(n) = note {
                let _ = app.emit_to("main", "status-note", n);
            }
        }
    }
}

/// Languages Tesseract can read here, for Settings.
#[tauri::command]
pub async fn ocr_languages(app: AppHandle) -> Result<Vec<String>, String> {
    let ocr = app.state::<AppState>().config().ocr;
    tauri::async_runtime::spawn_blocking(move || ocr.installed_languages()).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_inverts_dark_mode_and_upscales_small_crops() {
        let dark = RgbaImage::from_pixel(100, 40, image::Rgba([20, 20, 30, 255]));
        let out = prepare(&dark);
        assert_eq!((out.width(), out.height()), (300, 120));
        assert!(out.get_pixel(0, 0).0[0] > 200, "dark background becomes light");

        let light = RgbaImage::from_pixel(1000, 900, image::Rgba([250, 250, 250, 255]));
        let out = prepare(&light);
        assert_eq!((out.width(), out.height()), (1000, 900));
        assert!(out.get_pixel(0, 0).0[0] > 200);
    }
}
