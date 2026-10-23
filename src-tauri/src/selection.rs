//! Reading the user's selection and pasting text into the focused app.
//!
//! Linux reads the PRIMARY selection directly, which needs no key simulation. Windows and
//! macOS have no such selection, so we simulate copy and read the clipboard, restoring the
//! user's clipboard afterwards.

use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

/// A long-lived clipboard handle. On X11 the clipboard contents we set only stay available
/// while a handle exists, so it must not be dropped after each write.
pub struct ClipboardState(Mutex<Option<arboard::Clipboard>>);

impl ClipboardState {
    pub fn new() -> Self {
        match arboard::Clipboard::new() {
            Ok(c) => Self(Mutex::new(Some(c))),
            Err(e) => {
                tracing::warn!("clipboard unavailable: {e}");
                Self(Mutex::new(None))
            }
        }
    }

    fn with<T>(&self, f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self.0.lock().map_err(|_| "clipboard lock poisoned".to_string())?;
        let clip = guard.as_mut().ok_or("clipboard unavailable")?;
        f(clip)
    }

    /// Blocking: may simulate key presses and wait for the target app to respond.
    pub fn read_selection(&self) -> Result<String, String> {
        let text = self.with(read_selection)?;
        let text = text.trim().to_string();
        if text.is_empty() {
            Err("No text is selected".into())
        } else {
            Ok(text)
        }
    }

    /// Blocking: replaces the focused app's selection with `text` via a simulated paste.
    pub fn paste(&self, text: &str) -> Result<(), String> {
        self.with(|clip| {
            let saved = clip.get_text().ok();
            clip.set_text(text).map_err(|e| e.to_string())?;
            send_shortcut('v')?;
            // The target app fetches clipboard data asynchronously; restore only after it has.
            sleep(Duration::from_millis(400));
            if let Some(saved) = saved {
                let _ = clip.set_text(saved);
            }
            Ok(())
        })
    }
}

#[cfg(target_os = "linux")]
fn read_selection(clip: &mut arboard::Clipboard) -> Result<String, String> {
    use arboard::{GetExtLinux, LinuxClipboardKind};
    clip.get().clipboard(LinuxClipboardKind::Primary).text().map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
fn read_selection(clip: &mut arboard::Clipboard) -> Result<String, String> {
    let saved = clip.get_text().ok();
    let _ = clip.clear();
    send_shortcut('c')?;
    let mut text = Err("No text is selected".to_string());
    for _ in 0..25 {
        sleep(Duration::from_millis(20));
        if let Ok(t) = clip.get_text() {
            if !t.is_empty() {
                text = Ok(t);
                break;
            }
        }
    }
    if let Some(saved) = saved {
        let _ = clip.set_text(saved);
    }
    text
}

/// Press Ctrl/Cmd+`key`. Modifiers still held from the global shortcut (e.g. Alt) are released
/// first, otherwise the target app would see Ctrl+Alt+V instead of Ctrl+V.
fn send_shortcut(key: char) -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("cannot simulate keys: {e}"))?;
    for held in [Key::Alt, Key::Shift, Key::Meta, Key::Control] {
        let _ = enigo.key(held, Direction::Release);
    }
    let modifier = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
    let err = |e: enigo::InputError| e.to_string();
    enigo.key(modifier, Direction::Press).map_err(err)?;
    let result = enigo.key(Key::Unicode(key), Direction::Click).map_err(err);
    enigo.key(modifier, Direction::Release).map_err(err)?;
    result
}
