use regex::Regex;
use std::sync::LazyLock;

// Filler word regexes (pure hesitation markers — no semantic ambiguity)
// FR fillers include EN hesitations (uh, um) — ASR often transcribes French
// hesitations as English. These are not valid French words so safe to strip.
static RE_FILLERS_FR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(euh|heu|hum|bah|ben|beh|uh|um|hmm)\b").unwrap());
// EN fillers include FR hesitations (euh, heu) — same cross-language ASR issue.
static RE_FILLERS_EN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(uh|um|hmm|euh|heu)\b").unwrap());
static RE_FILLERS_DE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\u{00E4}h|\u{00E4}hm|hm|hmm|tja|naja)\b").unwrap());
static RE_FILLERS_ES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(eh|em|este|pues)\b").unwrap());
static RE_FILLERS_PT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(hum|tipo|n\u{00E9})\b").unwrap());
static RE_FILLERS_IT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(ehm|allora|cio\u{00E8}|ecco)\b").unwrap());
static RE_FILLERS_NL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(eh|ehm|uhm|nou)\b").unwrap());
static RE_FILLERS_PL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(yyy|eee|no|jakby)\b").unwrap());
static RE_FILLERS_RU: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(\u{044D}|\u{044D}\u{043C}|\u{043D}\u{0443}|\u{0432}\u{043E}\u{0442}|\u{0442}\u{0438}\u{043F}\u{0430}|\u{043A}\u{0430}\u{043A} \u{0431}\u{044B})\b").unwrap());
static RE_MULTI_SPACES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"  +").unwrap());

/// Extract base language code: "fr-CA" → "fr", "en" → "en"
fn lang_base(language: &str) -> &str {
    language.split(&['-', '_'][..]).next().unwrap_or(language)
}

/// Strip pure hesitation fillers (euh, uh, um, etc.) — no semantic ambiguity.
pub(super) fn strip_fillers(text: &str, language: &str) -> String {
    let re = match lang_base(language) {
        "fr" => &*RE_FILLERS_FR,
        "de" => &*RE_FILLERS_DE,
        "es" => &*RE_FILLERS_ES,
        "pt" => &*RE_FILLERS_PT,
        "it" => &*RE_FILLERS_IT,
        "nl" => &*RE_FILLERS_NL,
        "pl" => &*RE_FILLERS_PL,
        "ru" => &*RE_FILLERS_RU,
        _ => &*RE_FILLERS_EN,
    };
    let result = re.replace_all(text, "").to_string();
    let result = RE_MULTI_SPACES.replace_all(&result, " ").to_string();
    result.trim().to_string()
}
