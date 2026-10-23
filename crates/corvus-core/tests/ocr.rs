//! Runs the real Tesseract binary when it is installed; skipped otherwise.

use corvus_core::ocr::{join_lines, OcrConfig};

#[test]
fn recognizes_the_fixture() {
    let cfg = OcrConfig { languages: vec!["eng".into()], tesseract: None };
    match cfg.installed_languages() {
        Ok(langs) if langs.contains(&"eng".to_string()) => {}
        _ => return eprintln!("skipping: tesseract with eng data not installed"),
    }
    let png = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ocr-sample.png")).unwrap();
    let text = join_lines(&cfg.recognize(&png).unwrap());
    assert_eq!(text, "The quick brown fox jumps over the lazy dog. Optical character recognition works on screenshots.");
}

#[test]
fn missing_language_is_explained() {
    let cfg = OcrConfig { languages: vec!["xx-not-a-lang".into()], tesseract: None };
    if cfg.installed_languages().is_err() {
        return;
    }
    let err = cfg.recognize(b"").unwrap_err();
    assert!(err.contains("not installed: xx-not-a-lang"), "{err}");
}

#[test]
fn missing_binary_is_explained() {
    let cfg = OcrConfig { languages: vec![], tesseract: Some("/nonexistent/tesseract".into()) };
    assert!(cfg.installed_languages().unwrap_err().contains("Tesseract is not installed"));
}
