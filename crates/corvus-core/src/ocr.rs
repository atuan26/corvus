//! Text recognition with the Tesseract command-line tool. Runs locally; images never leave the
//! machine. Using the CLI (not libtesseract bindings) avoids build-time dependencies.

use std::io::Write;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OcrConfig {
    /// Tesseract language codes (e.g. `eng`, `vie`). Empty means every installed language.
    pub languages: Vec<String>,
    /// Path to the `tesseract` binary when it isn't on PATH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tesseract: Option<String>,
}

fn default_binary() -> &'static str {
    // The usual Windows installer (UB Mannheim) doesn't add itself to PATH.
    #[cfg(windows)]
    {
        const INSTALLED: &str = r"C:\Program Files\Tesseract-OCR\tesseract.exe";
        if std::path::Path::new(INSTALLED).exists() {
            return INSTALLED;
        }
    }
    "tesseract"
}

impl OcrConfig {
    fn binary(&self) -> &str {
        self.tesseract.as_deref().filter(|p| !p.is_empty()).unwrap_or_else(|| default_binary())
    }

    fn command(&self) -> Command {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut cmd = Command::new(self.binary());
        // A GUI app spawning a console program would otherwise flash a console window.
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
        cmd
    }

    /// Installed Tesseract language packs, without the `osd` orientation model.
    pub fn installed_languages(&self) -> Result<Vec<String>, String> {
        let out = self
            .command()
            .arg("--list-langs")
            .output()
            .map_err(|e| not_installed(self.binary(), e))?;
        // Older versions print the list to stderr.
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let mut langs: Vec<String> = text
            .lines()
            .skip_while(|l| !l.starts_with("List of available languages"))
            .skip(1)
            .map(str::trim)
            .filter(|l| !l.is_empty() && *l != "osd")
            .map(str::to_string)
            .collect();
        langs.sort();
        Ok(langs)
    }

    fn language_arg(&self) -> Result<String, String> {
        let installed = self.installed_languages()?;
        let chosen: Vec<&String> = if self.languages.is_empty() {
            installed.iter().collect()
        } else {
            let missing: Vec<&String> = self.languages.iter().filter(|l| !installed.contains(l)).collect();
            if !missing.is_empty() {
                return Err(format!(
                    "Tesseract language data not installed: {} (on Arch: pacman -S tesseract-data-<code>)",
                    missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
            self.languages.iter().collect()
        };
        if chosen.is_empty() {
            return Err("No Tesseract language data is installed".into());
        }
        Ok(chosen.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("+"))
    }

    /// Recognize text in an encoded image (PNG, etc.). Blocking.
    pub fn recognize(&self, image: &[u8]) -> Result<String, String> {
        let langs = self.language_arg()?;
        let mut child = self
            .command()
            .args(["stdin", "stdout", "-l", &langs, "--psm", "3"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| not_installed(self.binary(), e))?;
        // Write on another thread so a full stdout pipe can't deadlock us.
        let mut stdin = child.stdin.take().expect("piped");
        let data = image.to_vec();
        let writer = std::thread::spawn(move || stdin.write_all(&data));
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        let _ = writer.join();
        if !out.status.success() {
            let mut err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            err.truncate(300);
            return Err(format!("Tesseract failed: {err}"));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

fn not_installed(binary: &str, e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        format!("Tesseract is not installed (\"{binary}\" not found). Install Tesseract to use OCR.")
    } else {
        e.to_string()
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF)
}

/// Rejoin lines that OCR broke inside a paragraph, so translation sees whole sentences.
/// Blank lines stay as paragraph breaks; "recog-\nnition" becomes "recognition";
/// CJK lines join without a space.
pub fn join_lines(text: &str) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    for block in text.split("\n\n").map(str::trim).filter(|b| !b.is_empty()) {
        let mut out = String::new();
        for line in block.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if out.is_empty() {
                out.push_str(line);
                continue;
            }
            let prev = out.chars().last().unwrap_or(' ');
            let next = line.chars().next().unwrap_or(' ');
            if prev == '-' && out.chars().rev().nth(1).is_some_and(char::is_alphabetic) && next.is_lowercase() {
                out.pop();
            } else if !(is_cjk(prev) && is_cjk(next)) {
                out.push(' ');
            }
            out.push_str(line);
        }
        paragraphs.push(out);
    }
    paragraphs.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_wrapped_lines_and_hyphens() {
        let raw = "The quick brown fox jumps over the\nlazy dog. Optical character recog-\nnition works.\n\nSecond paragraph\nhere.";
        assert_eq!(
            join_lines(raw),
            "The quick brown fox jumps over the lazy dog. Optical character recognition works.\n\nSecond paragraph here."
        );
    }

    #[test]
    fn keeps_real_dashes_and_cjk() {
        assert_eq!(join_lines("well -\nknown"), "well - known");
        assert_eq!(join_lines("Anh-\nViệt"), "Anh- Việt");
        assert_eq!(join_lines("日本語の\n文章です"), "日本語の文章です");
    }
}
