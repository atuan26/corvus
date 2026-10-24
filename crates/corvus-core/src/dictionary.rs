//! Dictionary extras for a word or short phrase: pronunciation, meanings by part of speech,
//! examples. Google fills most of it alongside the translation; Wiktionary adds real IPA.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ProviderError;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Details {
    /// How the source is pronounced: Google's phonetic respelling for English, romanization for
    /// non-Latin scripts. Not IPA; see [`Wiktionary`] for that.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_phonetic: Option<String>,
    /// Romanization of the translation, for targets in non-Latin scripts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_phonetic: Option<String>,
    /// Spelling the provider suggests when the source looks mistyped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected: Option<String>,
    #[serde(default)]
    pub meanings: Vec<Meaning>,
    /// Usage examples in the source language.
    #[serde(default)]
    pub examples: Vec<String>,
}

impl Details {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Everything known about one part of speech, e.g. "verb".
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Meaning {
    pub pos: String,
    /// Other ways to translate the source with this part of speech, most common first.
    #[serde(default)]
    pub translations: Vec<Alternative>,
    /// Definitions in the source language.
    #[serde(default)]
    pub senses: Vec<Sense>,
    #[serde(default)]
    pub synonyms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Alternative {
    pub word: String,
    /// Source-language words that translate back to `word`.
    #[serde(default)]
    pub back: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sense {
    pub gloss: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

/// One IPA transcription, e.g. `/ɹʌn/` for "GA, RP".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ipa {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

/// Returns the meaning for `pos`, adding it at the end if it is new.
pub(crate) fn meaning_mut<'a>(meanings: &'a mut Vec<Meaning>, pos: &str) -> &'a mut Meaning {
    let i = match meanings.iter().position(|m| m.pos == pos) {
        Some(i) => i,
        None => {
            meanings.push(Meaning { pos: pos.into(), ..Default::default() });
            meanings.len() - 1
        }
    };
    &mut meanings[i]
}

pub const WIKTIONARY_URL: &str = "https://en.wiktionary.org";

/// IPA from English Wiktionary, which transcribes words of many languages with `{{IPA|<lang>|…}}`.
/// Languages that generate IPA from their own templates (Chinese, Japanese, French…) get nothing.
pub struct Wiktionary {
    base_url: String,
    client: reqwest::Client,
}

impl Wiktionary {
    pub fn new(base_url: Option<String>) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| WIKTIONARY_URL.into()),
            client: crate::providers::http_client(),
        }
    }

    /// `lang` is a provider language code such as "en" or "zh-CN". Unknown words give an empty list.
    pub async fn ipa(&self, word: &str, lang: &str) -> Result<Vec<Ipa>, ProviderError> {
        let word = word.trim();
        let code = wiktionary_code(lang);
        let found = self.ipa_for_page(word, &code).await?;
        // Page titles are case-sensitive; "Run" at the start of a sentence lives on "run".
        let lower = word.to_lowercase();
        if found.is_empty() && lower != word {
            return self.ipa_for_page(&lower, &code).await;
        }
        Ok(found)
    }

    async fn ipa_for_page(&self, title: &str, code: &str) -> Result<Vec<Ipa>, ProviderError> {
        let resp = self
            .client
            .get(format!("{}/w/api.php", self.base_url))
            .query(&[
                ("action", "parse"),
                ("page", title),
                ("prop", "wikitext"),
                ("format", "json"),
                ("formatversion", "2"),
                ("redirects", "1"),
            ])
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ProviderError::from_response(resp).await);
        }
        let body: Value = resp.json().await?;
        // A missing page is reported as {"error": {"code": "missingtitle"}}.
        Ok(body["parse"]["wikitext"].as_str().map(|w| parse_ipa(w, code)).unwrap_or_default())
    }
}

/// Wiktionary uses plain ISO 639 codes, some of which Google spells differently.
fn wiktionary_code(lang: &str) -> String {
    let base = lang.split(['-', '_']).next().unwrap_or(lang).to_lowercase();
    match base.as_str() {
        "iw" => "he".into(),
        "jw" => "jv".into(),
        _ => base,
    }
}

fn parse_ipa(wikitext: &str, code: &str) -> Vec<Ipa> {
    let open = format!("{{{{IPA|{code}|");
    let mut out: Vec<Ipa> = Vec::new();
    let mut rest = wikitext;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        let Some(end) = after.find("}}") else { break };
        let mut texts = Vec::new();
        let mut accent = None;
        for arg in after[..end].split('|').map(str::trim) {
            match arg.split_once('=') {
                Some(("a", v)) => accent = Some(v.split(',').map(str::trim).collect::<Vec<_>>().join(", ")),
                Some(_) => {}
                None if !arg.is_empty() => texts.push(arg.to_string()),
                None => {}
            }
        }
        for text in texts {
            if !out.iter().any(|i| i.text == text) {
                out.push(Ipa { text, accent: accent.clone() });
            }
        }
        rest = &after[end..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: &str = "==English==\n===Pronunciation===\n* {{IPA|en|/ɹʌn/|a=GA,RP}}\n\
        ** {{audio|en|en-us-run.ogg|a=US}}\n* {{IPA|en|/ɹʊn/|a=North England,Ireland}}\n\
        * {{IPA|en|/ɹʌn/|[ɾʌn]|a=Scotland}}\n==Dutch==\n* {{IPA|nl|/rʏn/}}\n";

    #[test]
    fn reads_ipa_for_the_language_only() {
        let ipa = parse_ipa(RUN, "en");
        assert_eq!(
            ipa,
            vec![
                Ipa { text: "/ɹʌn/".into(), accent: Some("GA, RP".into()) },
                Ipa { text: "/ɹʊn/".into(), accent: Some("North England, Ireland".into()) },
                Ipa { text: "[ɾʌn]".into(), accent: Some("Scotland".into()) },
            ]
        );
        assert_eq!(parse_ipa(RUN, "nl"), vec![Ipa { text: "/rʏn/".into(), accent: None }]);
        assert!(parse_ipa(RUN, "fr").is_empty());
    }

    #[test]
    fn maps_google_codes() {
        assert_eq!(wiktionary_code("zh-CN"), "zh");
        assert_eq!(wiktionary_code("iw"), "he");
        assert_eq!(wiktionary_code("EN"), "en");
    }
}
