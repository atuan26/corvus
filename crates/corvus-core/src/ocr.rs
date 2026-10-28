//! Text recognition with the Tesseract command-line tool. Runs locally; images never leave the
//! machine. Using the CLI (not libtesseract bindings) avoids build-time dependencies.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// Fast models from the official tessdata_fast repository, pinned to a commit.
const TESSDATA_URL: &str =
    "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/87416418657359cb625c412a48b6e1d6d41c29bd";

/// Every language tessdata_fast has (except `osd`, the orientation model), with English names.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("afr", "Afrikaans"), ("amh", "Amharic"), ("ara", "Arabic"), ("asm", "Assamese"), ("aze", "Azerbaijani"),
    ("aze_cyrl", "Azerbaijani (Cyrillic)"), ("bel", "Belarusian"), ("ben", "Bengali"), ("bod", "Tibetan"),
    ("bos", "Bosnian"), ("bre", "Breton"), ("bul", "Bulgarian"), ("cat", "Catalan"), ("ceb", "Cebuano"),
    ("ces", "Czech"), ("chi_sim", "Chinese (Simplified)"), ("chi_sim_vert", "Chinese (Simplified, vertical)"),
    ("chi_tra", "Chinese (Traditional)"), ("chi_tra_vert", "Chinese (Traditional, vertical)"), ("chr", "Cherokee"),
    ("cos", "Corsican"), ("cym", "Welsh"), ("dan", "Danish"), ("deu", "German"), ("deu_latf", "German (Fraktur)"),
    ("div", "Dhivehi"), ("dzo", "Dzongkha"), ("ell", "Greek"), ("eng", "English"), ("enm", "English (Middle)"),
    ("epo", "Esperanto"), ("equ", "Math equations"), ("est", "Estonian"), ("eus", "Basque"), ("fao", "Faroese"),
    ("fas", "Persian"), ("fil", "Filipino"), ("fin", "Finnish"), ("fra", "French"), ("frk", "German (Fraktur, legacy)"),
    ("frm", "French (Middle)"), ("fry", "Western Frisian"), ("gla", "Scottish Gaelic"), ("gle", "Irish"),
    ("glg", "Galician"), ("grc", "Greek (Ancient)"), ("guj", "Gujarati"), ("hat", "Haitian Creole"),
    ("heb", "Hebrew"), ("hin", "Hindi"), ("hrv", "Croatian"), ("hun", "Hungarian"), ("hye", "Armenian"),
    ("iku", "Inuktitut"), ("ind", "Indonesian"), ("isl", "Icelandic"), ("ita", "Italian"), ("ita_old", "Italian (Old)"),
    ("jav", "Javanese"), ("jpn", "Japanese"), ("jpn_vert", "Japanese (vertical)"), ("kan", "Kannada"),
    ("kat", "Georgian"), ("kat_old", "Georgian (Old)"), ("kaz", "Kazakh"), ("khm", "Khmer"), ("kir", "Kyrgyz"),
    ("kmr", "Kurdish (Kurmanji)"), ("kor", "Korean"), ("kor_vert", "Korean (vertical)"), ("lao", "Lao"),
    ("lat", "Latin"), ("lav", "Latvian"), ("lit", "Lithuanian"), ("ltz", "Luxembourgish"), ("mal", "Malayalam"),
    ("mar", "Marathi"), ("mkd", "Macedonian"), ("mlt", "Maltese"), ("mon", "Mongolian"), ("mri", "Maori"),
    ("msa", "Malay"), ("mya", "Burmese"), ("nep", "Nepali"), ("nld", "Dutch"), ("nor", "Norwegian"),
    ("oci", "Occitan"), ("ori", "Odia"), ("pan", "Punjabi"), ("pol", "Polish"), ("por", "Portuguese"),
    ("pus", "Pashto"), ("que", "Quechua"), ("ron", "Romanian"), ("rus", "Russian"), ("san", "Sanskrit"),
    ("sin", "Sinhala"), ("slk", "Slovak"), ("slv", "Slovenian"), ("snd", "Sindhi"), ("spa", "Spanish"),
    ("spa_old", "Spanish (Old)"), ("sqi", "Albanian"), ("srp", "Serbian"), ("srp_latn", "Serbian (Latin)"),
    ("sun", "Sundanese"), ("swa", "Swahili"), ("swe", "Swedish"), ("syr", "Syriac"), ("tam", "Tamil"),
    ("tat", "Tatar"), ("tel", "Telugu"), ("tgk", "Tajik"), ("tha", "Thai"), ("tir", "Tigrinya"), ("ton", "Tongan"),
    ("tur", "Turkish"), ("uig", "Uyghur"), ("ukr", "Ukrainian"), ("urd", "Urdu"), ("uzb", "Uzbek"),
    ("uzb_cyrl", "Uzbek (Cyrillic)"), ("vie", "Vietnamese"), ("yid", "Yiddish"), ("yor", "Yoruba"),
];

/// UB Mannheim's Windows build, the one Tesseract's documentation points to, checked against
/// this hash before it runs.
const WINDOWS_INSTALLER_URL: &str = "https://github.com/UB-Mannheim/tesseract/releases/download/v5.4.0.20240606/tesseract-ocr-w64-setup-5.4.0.20240606.exe";
const WINDOWS_INSTALLER_SHA256: &str = "c885fff6998e0608ba4bb8ab51436e1c6775c2bafc2559a19b423e18678b60c9";

/// Where Corvus keeps the language data it downloads, so adding a language needs no admin rights.
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("corvus").join("tessdata"))
}

/// A language Tesseract could read, for Settings.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OcrLanguage {
    pub code: String,
    pub name: String,
    /// Usable now, from the system's Tesseract data or Corvus's own.
    pub installed: bool,
    /// In Corvus's own folder, so Corvus can remove it.
    pub downloaded: bool,
}

/// Language data in the system's tessdata folder and in Corvus's.
struct Inventory {
    system_dir: Option<PathBuf>,
    system: Vec<String>,
    downloaded: Vec<String>,
}

impl Inventory {
    fn all(&self) -> Vec<String> {
        let mut all: Vec<String> = self.system.iter().chain(&self.downloaded).cloned().collect();
        all.sort();
        all.dedup();
        all
    }
}

fn traineddata_in(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".traineddata").filter(|c| *c != "osd").map(String::from)
        })
        .collect()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OcrConfig {
    /// Tesseract language codes (e.g. `eng`, `vie`). Empty means every installed language.
    pub languages: Vec<String>,
    /// Path to the `tesseract` binary when it isn't on PATH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tesseract: Option<String>,
}

fn default_binary() -> String {
    // The usual Windows installer (UB Mannheim) doesn't add itself to PATH, and apps started from
    // the macOS Dock don't see Homebrew's PATH.
    #[cfg(windows)]
    let candidates: Vec<PathBuf> = {
        let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
        let user_programs = std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Programs"));
        [program_files, user_programs]
            .into_iter()
            .flatten()
            .map(|d| d.join("Tesseract-OCR").join("tesseract.exe"))
            .collect()
    };
    #[cfg(target_os = "macos")]
    let candidates: Vec<PathBuf> = vec!["/opt/homebrew/bin/tesseract".into(), "/usr/local/bin/tesseract".into()];
    #[cfg(not(any(windows, target_os = "macos")))]
    let candidates: Vec<PathBuf> = Vec::new();
    candidates
        .into_iter()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tesseract".into())
}

impl OcrConfig {
    fn binary(&self) -> String {
        self.tesseract.clone().filter(|p| !p.is_empty()).unwrap_or_else(default_binary)
    }

    fn command(&self) -> Command {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut cmd = Command::new(self.binary());
        // A GUI app spawning a console program would otherwise flash a console window.
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
        cmd
    }

    /// Whether the Tesseract program can be run.
    pub fn is_installed(&self) -> bool {
        self.command().arg("--version").output().is_ok_and(|o| o.status.success())
    }

    fn inventory(&self) -> Result<Inventory, String> {
        let out = self
            .command()
            .arg("--list-langs")
            .output()
            .map_err(|e| not_installed(&self.binary(), e))?;
        // Older versions print the list to stderr.
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let header = text.lines().find(|l| l.starts_with("List of available languages"));
        // `List of available languages in "/usr/share/tessdata/" (3):`
        let system_dir = header
            .and_then(|h| Some(&h[h.find('"')? + 1..h.rfind('"')?]))
            .filter(|d| !d.is_empty())
            .map(PathBuf::from);
        let system = text
            .lines()
            .skip_while(|l| !l.starts_with("List of available languages"))
            .skip(1)
            .map(str::trim)
            .filter(|l| !l.is_empty() && *l != "osd")
            .map(str::to_string)
            .collect();
        let downloaded = data_dir().map(|d| traineddata_in(&d)).unwrap_or_default();
        Ok(Inventory { system_dir, system, downloaded })
    }

    /// Languages Tesseract can read now, from the system's data or Corvus's downloads.
    pub fn installed_languages(&self) -> Result<Vec<String>, String> {
        Ok(self.inventory()?.all())
    }

    /// Every known language, marked with whether it is installed. Installed languages that
    /// aren't in [`LANGUAGES`] (custom models) are included too.
    pub fn languages(&self) -> Result<Vec<OcrLanguage>, String> {
        let inv = self.inventory()?;
        let installed = inv.all();
        let mut list: Vec<OcrLanguage> = LANGUAGES
            .iter()
            .map(|(code, name)| (code.to_string(), name.to_string()))
            .chain(installed.iter().filter(|c| !LANGUAGES.iter().any(|(k, _)| k == c)).map(|c| (c.clone(), c.clone())))
            .map(|(code, name)| OcrLanguage {
                installed: installed.contains(&code),
                downloaded: inv.downloaded.contains(&code),
                code,
                name,
            })
            .collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(list)
    }

    /// The `-l` value and, when any language comes from Corvus's folder, the `--tessdata-dir`.
    /// Tesseract reads one folder, so system languages used alongside downloaded ones are copied
    /// into Corvus's folder the first time.
    fn plan(&self) -> Result<(String, Option<PathBuf>), String> {
        let inv = self.inventory()?;
        let all = inv.all();
        let chosen: Vec<String> = if self.languages.is_empty() {
            all
        } else {
            let missing: Vec<&String> = self.languages.iter().filter(|l| !all.contains(l)).collect();
            if !missing.is_empty() {
                return Err(format!(
                    "OCR language data not installed: {}. Add it in Settings → Screen text.",
                    missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
            self.languages.clone()
        };
        if chosen.is_empty() {
            return Err("No OCR languages yet. Add one in Settings → Screen text.".into());
        }
        let arg = chosen.join("+");
        if chosen.iter().all(|l| inv.system.contains(l)) {
            return Ok((arg, None));
        }
        let dir = data_dir().ok_or("No data folder on this system")?;
        for lang in chosen.iter().filter(|l| !inv.downloaded.contains(l)) {
            let from = inv.system_dir.as_ref().map(|d| d.join(format!("{lang}.traineddata")));
            let from = from.filter(|f| f.exists()).ok_or_else(|| format!("Can't find the data for {lang}"))?;
            std::fs::copy(&from, dir.join(format!("{lang}.traineddata")))
                .map_err(|e| format!("Could not copy the data for {lang}: {e}"))?;
        }
        Ok((arg, Some(dir)))
    }

    /// Recognize text in an encoded image (PNG, etc.). Blocking.
    pub fn recognize(&self, image: &[u8]) -> Result<String, String> {
        let (langs, tessdata) = self.plan()?;
        let mut cmd = self.command();
        if let Some(dir) = tessdata {
            cmd.arg("--tessdata-dir").arg(dir);
        }
        let mut child = cmd
            .args(["stdin", "stdout", "-l", &langs, "--psm", "3"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| not_installed(&self.binary(), e))?;
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

/// Download a language from tessdata_fast into [`data_dir`].
pub async fn download_language(code: &str) -> Result<(), String> {
    if !LANGUAGES.iter().any(|(c, _)| *c == code) {
        return Err(format!("Unknown OCR language: {code}"));
    }
    let dir = data_dir().ok_or("No data folder on this system")?;
    let resp = crate::providers::http_client()
        .get(format!("{TESSDATA_URL}/{code}.traineddata"))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Download failed: {e}"))?;
    let data = resp.bytes().await.map_err(|e| format!("Download failed: {e}"))?;
    let code = code.to_string();
    tokio::task::spawn_blocking(move || save_language(&dir, &code, &data))
        .await
        .map_err(|e| e.to_string())?
}

/// Written beside and then renamed, so a half-written file never looks like a language.
fn save_language(dir: &Path, code: &str, data: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let part = dir.join(format!("{code}.traineddata.part"));
    std::fs::write(&part, data).map_err(|e| e.to_string())?;
    std::fs::rename(&part, dir.join(format!("{code}.traineddata"))).map_err(|e| e.to_string())
}

/// Download the Windows Tesseract installer into `dir` and verify it. Returns its path.
pub async fn download_windows_installer(dir: &Path) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let data = crate::providers::http_client()
        .get(WINDOWS_INSTALLER_URL)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Download failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("Download failed: {e}"))?;
    let hash: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
    if hash != WINDOWS_INSTALLER_SHA256 {
        return Err("The downloaded Tesseract installer didn't match the expected file, so it wasn't run".into());
    }
    let path = dir.join("tesseract-ocr-w64-setup.exe");
    std::fs::write(&path, &data).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Remove a language Corvus downloaded (or copied); system data is left alone.
pub fn remove_language(code: &str) -> Result<(), String> {
    if code.is_empty() || !code.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return Err(format!("Unknown OCR language: {code}"));
    }
    let dir = data_dir().ok_or("No data folder on this system")?;
    std::fs::remove_file(dir.join(format!("{code}.traineddata"))).map_err(|e| e.to_string())
}

fn not_installed(binary: &str, e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        format!("Tesseract is not installed (\"{binary}\" not found). Install it in Settings → Screen text.")
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
