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

#[cfg(test)]
mod tests {
    use super::*;

    // -- lang_base --

    #[test]
    fn lang_base_simple() {
        assert_eq!(lang_base("fr"), "fr");
        assert_eq!(lang_base("en"), "en");
    }

    #[test]
    fn lang_base_with_region() {
        assert_eq!(lang_base("fr-CA"), "fr");
        assert_eq!(lang_base("en-US"), "en");
        assert_eq!(lang_base("pt-BR"), "pt");
    }

    #[test]
    fn lang_base_with_underscore() {
        assert_eq!(lang_base("zh_TW"), "zh");
    }

    // -- strip_fillers --

    #[test]
    fn french_fillers_removed() {
        assert_eq!(strip_fillers("euh bonjour heu monde", "fr"), "bonjour monde");
        assert_eq!(strip_fillers("bah c'est bien ben oui", "fr"), "c'est bien oui");
    }

    #[test]
    fn english_fillers_removed() {
        assert_eq!(strip_fillers("uh hello um world", "en"), "hello world");
        assert_eq!(strip_fillers("hmm interesting", "en"), "interesting");
    }

    #[test]
    fn german_fillers_removed() {
        assert_eq!(strip_fillers("\u{00E4}h das ist \u{00E4}hm gut", "de"), "das ist gut");
    }

    #[test]
    fn spanish_fillers_removed() {
        assert_eq!(strip_fillers("eh hola mundo", "es"), "hola mundo");
    }

    #[test]
    fn italian_fillers_removed() {
        assert_eq!(strip_fillers("ehm ciao mondo", "it"), "ciao mondo");
    }

    #[test]
    fn dutch_fillers_removed() {
        assert_eq!(strip_fillers("eh hallo nou wereld", "nl"), "hallo wereld");
    }

    #[test]
    fn polish_fillers_removed() {
        assert_eq!(strip_fillers("yyy cześć eee świat", "pl"), "cześć świat");
    }

    #[test]
    fn no_fillers_unchanged() {
        assert_eq!(strip_fillers("bonjour le monde", "fr"), "bonjour le monde");
    }

    #[test]
    fn cross_language_fr_en_fillers() {
        // French regex also catches English hesitations (common in FR ASR)
        assert_eq!(strip_fillers("uh bonjour um", "fr"), "bonjour");
    }

    #[test]
    fn fillers_case_insensitive() {
        assert_eq!(strip_fillers("EUH bonjour HEU", "fr"), "bonjour");
    }

    #[test]
    fn multi_spaces_collapsed() {
        assert_eq!(strip_fillers("hello   euh   world", "fr"), "hello world");
    }

    #[test]
    fn only_fillers_returns_empty() {
        assert_eq!(strip_fillers("euh heu hum", "fr"), "");
    }

    #[test]
    fn region_code_routes_correctly() {
        assert_eq!(strip_fillers("euh bonjour", "fr-CA"), "bonjour");
        assert_eq!(strip_fillers("uh hello", "en-US"), "hello");
    }

    #[test]
    fn unknown_language_defaults_to_english() {
        assert_eq!(strip_fillers("uh hello um", "ja"), "hello");
    }
}
