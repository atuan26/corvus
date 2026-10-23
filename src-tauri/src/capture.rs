//! Screen capture of one monitor. X11 reads the root window directly; Wayland asks the desktop's
//! screenshot portal; Windows and macOS use xcap.

use image::RgbaImage;

/// A monitor's area in physical pixels.
#[derive(Debug, Clone, Copy)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[cfg(target_os = "linux")]
pub async fn capture(area: Area) -> Result<RgbaImage, String> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    if wayland {
        return portal(area).await;
    }
    tauri::async_runtime::spawn_blocking(move || x11(area)).await.map_err(|e| e.to_string())?
}

#[cfg(target_os = "linux")]
fn x11(area: Area) -> Result<RgbaImage, String> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder};

    let (conn, screen_num) = x11rb::connect(None).map_err(|e| format!("cannot connect to X: {e}"))?;
    let setup = conn.setup();
    let root = setup.roots[screen_num].root;
    let reply = conn
        .get_image(ImageFormat::Z_PIXMAP, root, area.x as i16, area.y as i16, area.width as u16, area.height as u16, !0)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| format!("screen capture failed: {e}"))?;
    let bpp = setup
        .pixmap_formats
        .iter()
        .find(|f| f.depth == reply.depth)
        .map(|f| f.bits_per_pixel)
        .unwrap_or(32);
    if bpp != 32 {
        return Err(format!("unsupported screen format ({bpp} bits per pixel)"));
    }
    let lsb = setup.image_byte_order == ImageOrder::LSB_FIRST;
    let mut rgba = Vec::with_capacity(reply.data.len());
    for px in reply.data.as_chunks::<4>().0 {
        // 24/32-bit TrueColor: B,G,R,x in LSB order.
        let (r, g, b) = if lsb { (px[2], px[1], px[0]) } else { (px[1], px[2], px[3]) };
        rgba.extend_from_slice(&[r, g, b, 255]);
    }
    RgbaImage::from_raw(area.width, area.height, rgba).ok_or_else(|| "screen capture size mismatch".into())
}

#[cfg(target_os = "linux")]
async fn portal(area: Area) -> Result<RgbaImage, String> {
    use ashpd::desktop::screenshot::Screenshot;
    let shot = Screenshot::request()
        .interactive(false)
        .modal(false)
        .send()
        .await
        .and_then(|r| r.response())
        .map_err(|e| format!("the desktop refused the screenshot: {e}"))?;
    let path = tauri::Url::parse(shot.uri().as_str())
        .ok()
        .and_then(|u| u.to_file_path().ok())
        .ok_or("the screenshot portal returned no file")?;
    let full = image::open(&path).map_err(|e| e.to_string())?.to_rgba8();
    let _ = std::fs::remove_file(&path);
    // The portal captures the whole desktop; keep the monitor under the cursor.
    let x = area.x.max(0) as u32;
    let y = area.y.max(0) as u32;
    if x + area.width <= full.width() && y + area.height <= full.height() {
        Ok(image::imageops::crop_imm(&full, x, y, area.width, area.height).to_image())
    } else {
        Ok(full)
    }
}

#[cfg(not(target_os = "linux"))]
pub async fn capture(area: Area) -> Result<RgbaImage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let monitor = xcap::Monitor::from_point(area.x + 1, area.y + 1).map_err(|e| e.to_string())?;
        let img = monitor.capture_image().map_err(|e| e.to_string())?;
        RgbaImage::from_raw(img.width(), img.height(), img.into_raw()).ok_or_else(|| "capture failed".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
