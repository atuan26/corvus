use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
    /// Name in the language itself, so people can search the way they'd write it.
    pub native: &'static str,
}

macro_rules! langs {
    ($($code:literal => $name:literal, $native:literal);* $(;)?) => {
        &[$(Language { code: $code, name: $name, native: $native }),*]
    };
}

/// Languages offered in the UI. Providers reject codes they don't support with an HTTP error,
/// which the router turns into a fallback.
pub const LANGUAGES: &[Language] = langs![
    "auto" => "Detect language", "";
    "af" => "Afrikaans", "Afrikaans";
    "sq" => "Albanian", "Shqip";
    "am" => "Amharic", "አማርኛ";
    "ar" => "Arabic", "العربية";
    "hy" => "Armenian", "Հայերեն";
    "az" => "Azerbaijani", "Azərbaycanca";
    "eu" => "Basque", "Euskara";
    "be" => "Belarusian", "Беларуская";
    "bn" => "Bengali", "বাংলা";
    "bs" => "Bosnian", "Bosanski";
    "bg" => "Bulgarian", "Български";
    "ca" => "Catalan", "Català";
    "zh-CN" => "Chinese (Simplified)", "简体中文";
    "zh-TW" => "Chinese (Traditional)", "繁體中文";
    "hr" => "Croatian", "Hrvatski";
    "cs" => "Czech", "Čeština";
    "da" => "Danish", "Dansk";
    "nl" => "Dutch", "Nederlands";
    "en" => "English", "English";
    "eo" => "Esperanto", "Esperanto";
    "et" => "Estonian", "Eesti";
    "fil" => "Filipino", "Filipino";
    "fi" => "Finnish", "Suomi";
    "fr" => "French", "Français";
    "gl" => "Galician", "Galego";
    "ka" => "Georgian", "ქართული";
    "de" => "German", "Deutsch";
    "el" => "Greek", "Ελληνικά";
    "gu" => "Gujarati", "ગુજરાતી";
    "ht" => "Haitian Creole", "Kreyòl ayisyen";
    "ha" => "Hausa", "Hausa";
    "he" => "Hebrew", "עברית";
    "hi" => "Hindi", "हिन्दी";
    "hu" => "Hungarian", "Magyar";
    "is" => "Icelandic", "Íslenska";
    "id" => "Indonesian", "Bahasa Indonesia";
    "ga" => "Irish", "Gaeilge";
    "it" => "Italian", "Italiano";
    "ja" => "Japanese", "日本語";
    "jv" => "Javanese", "Basa Jawa";
    "kn" => "Kannada", "ಕನ್ನಡ";
    "kk" => "Kazakh", "Қазақ тілі";
    "km" => "Khmer", "ខ្មែរ";
    "ko" => "Korean", "한국어";
    "ku" => "Kurdish", "Kurdî";
    "ky" => "Kyrgyz", "Кыргызча";
    "lo" => "Lao", "ລາວ";
    "la" => "Latin", "Latina";
    "lv" => "Latvian", "Latviešu";
    "lt" => "Lithuanian", "Lietuvių";
    "mk" => "Macedonian", "Македонски";
    "ms" => "Malay", "Bahasa Melayu";
    "ml" => "Malayalam", "മലയാളം";
    "mt" => "Maltese", "Malti";
    "mr" => "Marathi", "मराठी";
    "mn" => "Mongolian", "Монгол";
    "my" => "Myanmar (Burmese)", "မြန်မာ";
    "ne" => "Nepali", "नेपाली";
    "nb" => "Norwegian", "Norsk bokmål";
    "fa" => "Persian", "فارسی";
    "pl" => "Polish", "Polski";
    "pt" => "Portuguese", "Português";
    "pa" => "Punjabi", "ਪੰਜਾਬੀ";
    "ro" => "Romanian", "Română";
    "ru" => "Russian", "Русский";
    "sr" => "Serbian", "Српски";
    "si" => "Sinhala", "සිංහල";
    "sk" => "Slovak", "Slovenčina";
    "sl" => "Slovenian", "Slovenščina";
    "so" => "Somali", "Soomaali";
    "es" => "Spanish", "Español";
    "sw" => "Swahili", "Kiswahili";
    "sv" => "Swedish", "Svenska";
    "tg" => "Tajik", "Тоҷикӣ";
    "ta" => "Tamil", "தமிழ்";
    "te" => "Telugu", "తెలుగు";
    "th" => "Thai", "ไทย";
    "tr" => "Turkish", "Türkçe";
    "uk" => "Ukrainian", "Українська";
    "ur" => "Urdu", "اردو";
    "uz" => "Uzbek", "Oʻzbekcha";
    "vi" => "Vietnamese", "Tiếng Việt";
    "cy" => "Welsh", "Cymraeg";
    "xh" => "Xhosa", "isiXhosa";
    "yi" => "Yiddish", "ייִדיש";
    "yo" => "Yoruba", "Yorùbá";
    "zu" => "Zulu", "isiZulu";
];

pub fn name(code: &str) -> Option<&'static str> {
    LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(code)).map(|l| l.name)
}
