//! Installing Tesseract and its language data from Settings → Screen text.

use std::path::{Path, PathBuf};
use std::process::Command;

use corvus_core::ocr::{self, OcrLanguage};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;

/// How Settings can get Tesseract onto this system.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InstallMethod {
    /// Download the official installer and open it.
    WindowsInstaller,
    /// Run a command; on Linux it asks for the admin password through polkit.
    Command { program: String, args: Vec<String> },
    /// Nothing Corvus can run; show these instructions.
    Manual { command: Option<String>, url: String },
}

const DOWNLOAD_PAGE: &str = "https://tesseract-ocr.github.io/tessdoc/Installation.html";

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(program)).find(|p| p.is_file())
}

fn install_method() -> InstallMethod {
    if cfg!(windows) {
        return InstallMethod::WindowsInstaller;
    }
    if cfg!(target_os = "macos") {
        let brew = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"].into_iter().map(Path::new).find(|p| p.exists());
        return match brew {
            Some(brew) => InstallMethod::Command {
                program: brew.display().to_string(),
                args: vec!["install".into(), "tesseract".into()],
            },
            None => InstallMethod::Manual { command: Some("brew install tesseract".into()), url: "https://brew.sh".into() },
        };
    }
    // Package manager and the packages that give a working `tesseract` with English.
    let managers: [(&str, &[&str]); 4] = [
        ("apt-get", &["install", "-y", "tesseract-ocr"]),
        ("dnf", &["install", "-y", "tesseract"]),
        ("pacman", &["-S", "--needed", "--noconfirm", "tesseract", "tesseract-data-eng"]),
        ("zypper", &["--non-interactive", "install", "tesseract-ocr", "tesseract-ocr-traineddata-english"]),
    ];
    let Some((manager, args)) = managers.into_iter().find(|(m, _)| find_in_path(m).is_some()) else {
        return InstallMethod::Manual { command: None, url: DOWNLOAD_PAGE.into() };
    };
    match find_in_path("pkexec") {
        Some(pkexec) => InstallMethod::Command {
            program: pkexec.display().to_string(),
            args: std::iter::once(manager).chain(args.iter().copied()).map(String::from).collect(),
        },
        None => InstallMethod::Manual { command: Some(format!("sudo {manager} {}", args.join(" "))), url: DOWNLOAD_PAGE.into() },
    }
}

#[derive(Serialize)]
pub struct TesseractStatus {
    installed: bool,
    install: InstallMethod,
}

#[tauri::command]
pub async fn tesseract_status(app: AppHandle) -> Result<TesseractStatus, String> {
    let ocr = app.state::<AppState>().config().ocr;
    tauri::async_runtime::spawn_blocking(move || TesseractStatus { installed: ocr.is_installed(), install: install_method() })
        .await
        .map_err(|e| e.to_string())
}

/// `true` when Tesseract is ready; `false` when an installer window was opened for the user to
/// finish.
#[tauri::command]
pub async fn tesseract_install(app: AppHandle) -> Result<bool, String> {
    match install_method() {
        InstallMethod::WindowsInstaller => {
            let path = ocr::download_windows_installer(&std::env::temp_dir()).await?;
            // Opened through the shell so Windows shows its usual admin prompt.
            use tauri_plugin_opener::OpenerExt;
            app.opener().open_path(path.display().to_string(), None::<&str>).map_err(|e| e.to_string())?;
            Ok(false)
        }
        InstallMethod::Command { program, args } => {
            let pkexec = program.ends_with("pkexec");
            let manual = format!("sudo {}", args.join(" "));
            let out = tauri::async_runtime::spawn_blocking(move || Command::new(&program).args(&args).output())
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
            let err = String::from_utf8_lossy(&out.stderr);
            let lines: Vec<&str> = err.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            let detail = lines[lines.len().saturating_sub(3)..].join(" ");
            match out.status.code() {
                Some(0) => Ok(true),
                // pkexec: 126 is a dismissed or refused password prompt, 127 no prompt at all.
                Some(126) if pkexec => Err("Installation cancelled".into()),
                Some(127) if pkexec => {
                    Err(format!("Couldn't ask for the admin password ({detail}). Run this in a terminal instead: {manual}"))
                }
                _ => Err(format!("Installation failed: {detail}")),
            }
        }
        InstallMethod::Manual { .. } => Err("Install Tesseract with the command shown".into()),
    }
}

/// Every OCR language, marked installed or not. Fails when Tesseract isn't installed.
#[tauri::command]
pub async fn ocr_languages(app: AppHandle) -> Result<Vec<OcrLanguage>, String> {
    let ocr = app.state::<AppState>().config().ocr;
    tauri::async_runtime::spawn_blocking(move || ocr.languages()).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ocr_download_language(code: String) -> Result<(), String> {
    ocr::download_language(&code).await
}

#[tauri::command]
pub fn ocr_remove_language(code: String) -> Result<(), String> {
    ocr::remove_language(&code)
}
