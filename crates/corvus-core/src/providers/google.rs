//! Google Translate through the keyless `translate_a/single` endpoint used by the web widget.
//! Unofficial: it can be rate-limited, so it should never be the only provider in the chain.
//! For single words and short phrases it also returns dictionary data, which fills `details`.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::dictionary::{meaning_mut, Alternative, Details, Sense};
use crate::error::ProviderError;
use crate::provider::{ProviderInfo, TranslateRequest, Translation, Translator};

pub const DEFAULT_BASE_URL: &str = "https://translate.googleapis.com";

/// Translation, transliteration, dictionary, definitions, examples, synonyms, spelling.
/// Google leaves the dictionary parts out for longer text.
const DATA: [&str; 7] = ["t", "rm", "bd", "md", "ex", "ss", "qca"];

pub struct GoogleFree {
    id: String,
    base_url: String,
    client: reqwest::Client,
}

impl GoogleFree {
    pub fn new(id: impl Into<String>, base_url: Option<String>) -> Self {
        Self {
            id: id.into(),
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
            client: super::http_client(),
        }
    }
}

#[async_trait]
impl Translator for GoogleFree {
    fn info(&self) -> ProviderInfo {
        ProviderInfo { id: self.id.clone(), name: "Google".into(), kind: "google-free".into() }
    }

    async fn translate(&self, req: &TranslateRequest) -> Result<Translation, ProviderError> {
        let source = if req.is_auto() { "auto" } else { req.source.as_str() };
        // The dictionary extension's client id; "gtx" is throttled much sooner.
        // dj=1 returns an object with named keys instead of positional arrays.
        let mut query = vec![("client", "dict-chrome-ex"), ("dj", "1"), ("ie", "UTF-8"), ("oe", "UTF-8")];
        query.extend(DATA.iter().map(|d| ("dt", *d)));
        // POST keeps long inputs out of the URL, which the GET form truncates.
        let resp = self
            .client
            .post(format!("{}/translate_a/single", self.base_url))
            .query(&query)
            .form(&[("sl", source), ("tl", req.target.as_str()), ("q", req.text.as_str())])
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ProviderError::from_response(resp).await);
        }
        let body: Value = resp.json().await?;
        parse(&self.id, body)
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    sentences: Vec<Sentence>,
    src: Option<String>,
    spell: Option<Spell>,
    #[serde(default)]
    dict: Vec<PosGroup<DictEntry>>,
    #[serde(default)]
    definitions: Vec<PosGroup<Definition>>,
    #[serde(default)]
    synsets: Vec<PosGroup<Synset>>,
    examples: Option<Examples>,
}

/// Either a translated segment or, last in the list, the transliterations.
#[derive(Deserialize)]
struct Sentence {
    trans: Option<String>,
    translit: Option<String>,
    src_translit: Option<String>,
}

#[derive(Deserialize)]
struct Spell {
    spell_res: Option<String>,
}

#[derive(Deserialize)]
struct PosGroup<T> {
    #[serde(default)]
    pos: String,
    #[serde(default = "Vec::new")]
    entry: Vec<T>,
}

#[derive(Deserialize)]
struct DictEntry {
    word: String,
    #[serde(default)]
    reverse_translation: Vec<String>,
}

#[derive(Deserialize)]
struct Definition {
    gloss: String,
    example: Option<String>,
}

#[derive(Deserialize)]
struct Synset {
    #[serde(default)]
    synonym: Vec<String>,
}

#[derive(Deserialize)]
struct Examples {
    #[serde(default)]
    example: Vec<Example>,
}

#[derive(Deserialize)]
struct Example {
    text: String,
}

fn parse(id: &str, body: Value) -> Result<Translation, ProviderError> {
    let r: Response = serde_json::from_value(body).map_err(|e| ProviderError::Parse(e.to_string()))?;
    if !r.sentences.iter().any(|s| s.trans.is_some()) {
        return Err(ProviderError::Parse("missing sentences".into()));
    }
    let text: String = r.sentences.iter().filter_map(|s| s.trans.as_deref()).collect();
    let nonempty = |s: &Option<String>| s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from);

    let mut details = Details {
        source_phonetic: r.sentences.iter().find_map(|s| nonempty(&s.src_translit)),
        target_phonetic: r.sentences.iter().find_map(|s| nonempty(&s.translit)),
        corrected: r.spell.and_then(|s| nonempty(&s.spell_res)),
        meanings: Vec::new(),
        examples: r.examples.map(|e| e.example.into_iter().map(|x| plain(&x.text)).collect()).unwrap_or_default(),
    };
    for group in r.dict {
        let m = meaning_mut(&mut details.meanings, &group.pos);
        m.translations
            .extend(group.entry.into_iter().map(|e| Alternative { word: e.word, back: e.reverse_translation }));
    }
    for group in r.definitions {
        let m = meaning_mut(&mut details.meanings, &group.pos);
        m.senses.extend(group.entry.into_iter().map(|d| Sense { gloss: d.gloss, example: d.example }));
    }
    for group in r.synsets {
        let m = meaning_mut(&mut details.meanings, &group.pos);
        for word in group.entry.into_iter().flat_map(|s| s.synonym) {
            if !m.synonyms.contains(&word) {
                m.synonyms.push(word);
            }
        }
    }

    Ok(Translation {
        text,
        detected_source: r.src.map(|s| s.to_lowercase()),
        provider: id.into(),
        details: (!details.is_empty()).then_some(details),
    })
}

/// Examples mark the looked-up word with `<b>` and escape a few characters.
fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&#39;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::Meaning;
    use serde_json::json;

    #[test]
    fn joins_segments_and_reads_detected_language() {
        let body = json!({
            "sentences": [{ "trans": "Xin chào. ", "orig": "Hello. " }, { "trans": "Thế giới", "orig": "World" }],
            "src": "en",
            "spell": {}
        });
        let t = parse("google", body).unwrap();
        assert_eq!(t.text, "Xin chào. Thế giới");
        assert_eq!(t.detected_source.as_deref(), Some("en"));
        assert_eq!(t.details, None);
    }

    #[test]
    fn reads_dictionary_data() {
        // Trimmed from a real response for "run", en → vi.
        let body = json!({
            "sentences": [{ "trans": "chạy", "orig": "run" }, { "src_translit": "rən" }],
            "dict": [{ "pos": "verb", "terms": ["chạy", "trốn"], "entry": [
                { "word": "chạy", "reverse_translation": ["run", "go"], "score": 0.3 },
                { "word": "trốn", "reverse_translation": ["flee", "run"] }
            ]}],
            "src": "en",
            "spell": {},
            "synsets": [{ "pos": "verb", "entry": [
                { "synonym": ["sprint", "race"], "definition_id": "a" },
                { "synonym": ["race", "dash"], "definition_id": "b" }
            ]}],
            "definitions": [
                { "pos": "verb", "entry": [{ "gloss": "move at a speed faster than a walk.", "example": "the dog ran across the road" }] },
                { "pos": "noun", "entry": [{ "gloss": "an act or spell of running." }] }
            ],
            "examples": { "example": [{ "text": "Bobby set off at a <b>run</b>", "definition_id": "c" }, { "text": "we&#39;ll <b>run</b>" }] }
        });
        let d = parse("google", body).unwrap().details.unwrap();
        assert_eq!(d.source_phonetic.as_deref(), Some("rən"));
        assert_eq!(d.examples, ["Bobby set off at a run", "we'll run"]);
        assert_eq!(
            d.meanings,
            vec![
                Meaning {
                    pos: "verb".into(),
                    translations: vec![
                        Alternative { word: "chạy".into(), back: vec!["run".into(), "go".into()] },
                        Alternative { word: "trốn".into(), back: vec!["flee".into(), "run".into()] },
                    ],
                    senses: vec![Sense {
                        gloss: "move at a speed faster than a walk.".into(),
                        example: Some("the dog ran across the road".into()),
                    }],
                    synonyms: vec!["sprint".into(), "race".into(), "dash".into()],
                },
                Meaning {
                    pos: "noun".into(),
                    senses: vec![Sense { gloss: "an act or spell of running.".into(), example: None }],
                    ..Default::default()
                },
            ]
        );
    }

    #[test]
    fn reads_transliteration_and_spelling() {
        let body = json!({
            "sentences": [{ "trans": "今日はいい天気", "orig": "nice weather" }, { "translit": "Kyō wa ī tenki" }],
            "src": "en",
            "spell": { "spell_html_res": "<b><i>receive</i></b>", "spell_res": "receive", "correction_type": [1] }
        });
        let d = parse("google", body).unwrap().details.unwrap();
        assert_eq!(d.target_phonetic.as_deref(), Some("Kyō wa ī tenki"));
        assert_eq!(d.corrected.as_deref(), Some("receive"));
    }

    #[test]
    fn rejects_unexpected_shape() {
        assert!(matches!(parse("google", json!({"error": 1})), Err(ProviderError::Parse(_))));
        assert!(matches!(parse("google", json!([[["a", "b"]], null, "en"])), Err(ProviderError::Parse(_))));
    }
}
