use regex::Regex;
use std::sync::LazyLock;

// Punctuation spacing regexes (compiled once)
static RE_SPACE_BEFORE_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s+([.,?!;:\u{2026})\u{00BB}"\]])"#).unwrap());
static RE_SPACE_AFTER_PUNCT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([.,?!;:\u{2026}])([^\s\n.,?!;:\u{2026})\u{00BB}"\]\d])"#).unwrap());
static RE_SPACE_AFTER_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([\(\u{00AB}"\[])\s+"#).unwrap());
static RE_CAPITALIZE_AFTER_SENTENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([.?!]\s+|\n)(\p{Ll})").unwrap());

// Filler word regexes (pure hesitation markers — no semantic ambiguity)
static RE_FILLERS_FR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(euh|heu|hum|bah|ben|beh)\b").unwrap());
static RE_FILLERS_EN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(uh|um|hmm)\b").unwrap());
static RE_FILLERS_DE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(äh|ähm|hm|hmm|tja|naja)\b").unwrap());
static RE_FILLERS_ES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(eh|em|este|pues)\b").unwrap());
static RE_FILLERS_PT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(hum|tipo|né)\b").unwrap());
static RE_FILLERS_IT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(ehm|allora|cioè|ecco)\b").unwrap());
static RE_FILLERS_NL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(eh|ehm|uhm|nou)\b").unwrap());
static RE_FILLERS_PL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(yyy|eee|no|jakby)\b").unwrap());
static RE_FILLERS_RU: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(э|эм|ну|вот|типа|как бы)\b").unwrap());
static RE_MULTI_SPACES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"  +").unwrap());

pub struct PostProcessOptions {
    pub hallucination_filter: bool,
    pub disfluency_removal: bool,
}

/// Phase 1: hallucination filter + dictation command substitution.
pub fn preprocess(text: &str, language: &str, opts: &PostProcessOptions) -> String {
    let mut result = if opts.hallucination_filter {
        strip_hallucinations(text)
    } else {
        text.to_string()
    };
    if result.trim().is_empty() {
        return String::new();
    }

    let lang = resolve_language(language, &result);
    result = apply_dictation_commands(&result, &lang);

    if opts.disfluency_removal {
        result = strip_fillers(&result, &lang);
    }

    result
}

/// Phase 2: spacing fixes + capitalization after sentences + capitalize first char.
pub fn finalize(text: &str) -> String {
    let mut result = fix_punctuation_spacing(text);
    result = capitalize_after_sentence_endings(&result);

    // Capitalize first character
    if let Some(first) = result.chars().next() {
        if first.is_lowercase() {
            let upper: String = first.to_uppercase().collect();
            result = format!("{}{}", upper, &result[first.len_utf8()..]);
        }
    }

    result
}

/// Full pipeline: preprocess + finalize (convenience wrapper, used by tests).
#[cfg(test)]
pub fn process(text: &str, language: &str, opts: &PostProcessOptions) -> String {
    let preprocessed = preprocess(text, language, opts);
    if preprocessed.is_empty() {
        return String::new();
    }
    finalize(&preprocessed)
}

/// Known Whisper hallucination phrases that appear on silence/noise.
/// Organized by language, checked case-insensitively.
const HALLUCINATIONS: &[&str] = &[
    // -- Cross-language --
    "amara.org",
    "www.",
    "http",
    "♪",
    "♫",
    "...",
    "…",
    // -- French --
    "sous-titrage société radio-canada",
    "sous-titrage st",
    "sous titrage société radio canada",
    "soustitrage société radio-canada",
    "sous-titrage",
    "sous-titres par",
    "sous-titres réalisés par",
    "par soustitreur.com",
    "merci d'avoir regardé",
    "merci pour votre écoute",
    "au revoir.",
    "à bientôt.",
    // -- English --
    "subtitles by",
    "thank you for watching",
    "thanks for watching",
    "please subscribe",
    "like and subscribe",
    "don't forget to subscribe",
    "see you in the next video",
    "bye.",
    "bye bye.",
    "bye-bye.",
    // -- German --
    "untertitel im auftrag des zdf",
    "untertitel der amara.org-community",
    "vielen dank fürs zuschauen",
    "danke fürs zuschauen",
    "bis zum nächsten mal",
    "tschüss",
    // -- Spanish --
    "subtítulos realizados por",
    "subtitulado por",
    "gracias por ver",
    "suscríbete al canal",
    "no olvides suscribirte",
    // -- Portuguese --
    "legendas pela comunidade",
    "obrigado por assistir",
    "tchau",
    // -- Italian --
    "sottotitoli creati dalla comunità",
    "sottotitoli a cura di",
    "grazie per la visione",
    "grazie per aver guardato",
    // -- Dutch --
    "ondertiteld door",
    "ondertiteling door",
    "bedankt voor het kijken",
    // -- Polish --
    "napisy stworzone przez",
    "dziękuję za obejrzenie",
    "dziękuję za uwagę",
    // -- Russian --
    "субтитры сделаны сообществом",
    "спасибо за просмотр",
    "подписывайтесь на канал",
];

// Pre-compiled regexes for hallucination removal (case-insensitive)
static HALLUCINATION_REGEXES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    HALLUCINATIONS
        .iter()
        .map(|h| Regex::new(&format!("(?i){}", regex::escape(h))).unwrap())
        .collect()
});

// Music/symbol-only output
static RE_MUSIC_ONLY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\s♪♫🎵.\u{2026}]+$").unwrap());

/// Strip known Whisper hallucination phrases from text.
/// If only hallucinations remain, returns empty string.
fn strip_hallucinations(text: &str) -> String {
    let mut result = text.to_string();
    let lower = result.to_lowercase();
    let trimmed_lower = lower.trim().trim_matches('.').trim();

    // Music/symbol-only output
    if RE_MUSIC_ONLY.is_match(trimmed_lower) {
        log::info!("Filtered hallucination (music/symbols): {:?}", text.trim());
        return String::new();
    }

    // If the entire text (trimmed, case-insensitive) matches a hallucination, discard it
    for h in HALLUCINATIONS {
        if trimmed_lower == *h || trimmed_lower.starts_with(h) {
            log::info!("Filtered hallucination: {:?}", text.trim());
            return String::new();
        }
    }

    // Repetition detection: same word 3+ times in a row → likely looping
    if has_excessive_repetition(trimmed_lower) {
        log::info!("Filtered hallucination (repetition): {:?}", text.trim());
        return String::new();
    }

    // Remove hallucination phrases embedded in longer text
    for re in HALLUCINATION_REGEXES.iter() {
        result = re.replace_all(&result, "").to_string();
    }

    result
}

fn resolve_language(code: &str, text: &str) -> String {
    if code != "auto" {
        return code.to_string();
    }

    let french_words: &[&str] = &[
        "le", "la", "les", "de", "des", "du", "un", "une", "et", "est",
        "que", "qui", "dans", "pour", "pas", "sur", "avec", "tout", "mais", "comme",
    ];

    let lower = text.to_lowercase();
    let french_count = lower.split_whitespace().filter(|w| french_words.contains(w)).count();

    if french_count >= 2 { "fr".to_string() } else { "en".to_string() }
}

// Pre-compiled dictation command regexes per language
static DICTATION_COMMANDS_FR: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        ("point d'interrogation", "?"),
        ("point d'exclamation", "!"),
        ("points de suspension", "\u{2026}"),
        ("point-virgule", ";"),
        ("point virgule", ";"),
        ("deux-points", ":"),
        ("deux points", ":"),
        ("ouvrir la parenthèse", "("),
        ("fermer la parenthèse", ")"),
        ("ouvrir les guillemets", "«\u{00A0}"),
        ("fermer les guillemets", "\u{00A0}»"),
        ("à la ligne", "\n"),
        ("nouvelle ligne", "\n"),
        ("nouveau paragraphe", "\n\n"),
        ("virgule", ","),
        ("point", "."),
        ("tiret", "-"),
    ]
    .iter()
    .map(|(p, r)| (Regex::new(&format!("(?i){}", regex::escape(p))).unwrap(), *r))
    .collect()
});

static DICTATION_COMMANDS_EN: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        ("question mark", "?"),
        ("exclamation mark", "!"),
        ("exclamation point", "!"),
        ("semicolon", ";"),
        ("semi-colon", ";"),
        ("ellipsis", "\u{2026}"),
        ("colon", ":"),
        ("open parenthesis", "("),
        ("close parenthesis", ")"),
        ("open paren", "("),
        ("close paren", ")"),
        ("open quote", "\""),
        ("close quote", "\""),
        ("new line", "\n"),
        ("newline", "\n"),
        ("new paragraph", "\n\n"),
        ("comma", ","),
        ("period", "."),
        ("full stop", "."),
        ("dash", "-"),
        ("hyphen", "-"),
    ]
    .iter()
    .map(|(p, r)| (Regex::new(&format!("(?i){}", regex::escape(p))).unwrap(), *r))
    .collect()
});

fn apply_dictation_commands(text: &str, language: &str) -> String {
    let commands = if language.starts_with("fr") {
        &*DICTATION_COMMANDS_FR
    } else {
        &*DICTATION_COMMANDS_EN
    };

    let mut result = text.to_string();
    for (re, replacement) in commands {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

/// Strip pure hesitation fillers (euh, uh, um, etc.) — no semantic ambiguity.
fn strip_fillers(text: &str, language: &str) -> String {
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

/// Extract base language code: "fr-CA" → "fr", "en" → "en"
fn lang_base(language: &str) -> &str {
    language.split(&['-', '_'][..]).next().unwrap_or(language)
}

/// Detect excessive repetition (same word 3+ times in a row, or text is mostly one word).
/// Whisper hallucinates by looping the same word/phrase on silence.
fn has_excessive_repetition(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 3 {
        return false;
    }

    // Check for same word repeated 3+ times consecutively
    let mut run_count = 1;
    for i in 1..words.len() {
        if words[i].eq_ignore_ascii_case(words[i - 1]) {
            run_count += 1;
            if run_count >= 3 {
                return true;
            }
        } else {
            run_count = 1;
        }
    }

    // Check if a single word dominates (>70% of all words, at least 4 occurrences)
    let mut counts = std::collections::HashMap::new();
    for w in &words {
        *counts.entry(w.to_lowercase()).or_insert(0u32) += 1;
    }
    if let Some(&max_count) = counts.values().max() {
        if max_count >= 4 && (max_count as f32 / words.len() as f32) > 0.7 {
            return true;
        }
    }

    false
}

fn fix_punctuation_spacing(text: &str) -> String {
    let mut result = text.to_string();

    // Remove space before closing punctuation: "word ." → "word."
    result = RE_SPACE_BEFORE_CLOSE.replace_all(&result, "$1").to_string();

    // Ensure space after punctuation (except before newline, end, or more punctuation)
    result = RE_SPACE_AFTER_PUNCT.replace_all(&result, "$1 $2").to_string();

    // Remove space after opening punctuation: "( word" → "(word"
    result = RE_SPACE_AFTER_OPEN.replace_all(&result, "$1").to_string();

    result
}

fn capitalize_after_sentence_endings(text: &str) -> String {
    let result = RE_CAPITALIZE_AFTER_SENTENCE.replace_all(text, |caps: &regex::Captures| {
        let prefix = &caps[1];
        let letter = &caps[2];
        format!("{}{}", prefix, letter.to_uppercase())
    });
    result.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_opts() -> PostProcessOptions {
        PostProcessOptions { hallucination_filter: true, disfluency_removal: true }
    }

    #[test]
    fn test_french_commands() {
        let result = process("bonjour virgule comment allez-vous point d'interrogation", "fr", &default_opts());
        assert_eq!(result, "Bonjour, comment allez-vous?");
    }

    #[test]
    fn test_english_commands() {
        let result = process("hello comma how are you question mark", "en", &default_opts());
        assert_eq!(result, "Hello, how are you?");
    }

    #[test]
    fn test_auto_detect_french() {
        let result = process("le chat est dans la maison", "auto", &default_opts());
        assert!(result.starts_with("Le"));
    }

    #[test]
    fn test_capitalization() {
        let result = process("hello. world", "en", &default_opts());
        assert_eq!(result, "Hello. World");
    }

    #[test]
    fn test_hallucination_filter() {
        let opts = default_opts();
        assert_eq!(process("Sous-titrage Société Radio-Canada", "fr", &opts), "");
        assert_eq!(process("sous-titrage", "fr", &opts), "");
        assert_eq!(process("Thank you for watching", "en", &opts), "");
        assert_eq!(process("...", "en", &opts), "");
    }

    #[test]
    fn test_hallucination_filter_disabled() {
        let opts = PostProcessOptions { hallucination_filter: false, disfluency_removal: true };
        let result = process("sous-titrage", "fr", &opts);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_filler_removal_french() {
        let result = process("euh ben euh bonjour", "fr", &default_opts());
        assert_eq!(result, "Bonjour");
    }

    #[test]
    fn test_filler_removal_english() {
        let result = process("um hello uh world", "en", &default_opts());
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_filler_removal_disabled() {
        let opts = PostProcessOptions { hallucination_filter: true, disfluency_removal: false };
        let result = process("euh bonjour", "fr", &opts);
        assert!(result.to_lowercase().contains("euh"));
    }

    #[test]
    fn test_hallucination_embedded() {
        let result = process("bonjour sous-titrage tout le monde", "fr", &default_opts());
        assert!(result.contains("Bonjour"));
        assert!(!result.to_lowercase().contains("sous-titrage"));
    }

    // --- finalize ---

    #[test]
    fn test_finalize_space_before_punct() {
        assert_eq!(finalize("hello ."), "Hello.");
    }

    #[test]
    fn test_finalize_space_after_punct() {
        assert_eq!(finalize("hello.world"), "Hello. World");
    }

    #[test]
    fn test_finalize_capitalize_first() {
        assert_eq!(finalize("hello"), "Hello");
    }

    #[test]
    fn test_finalize_capitalize_after_question() {
        assert_eq!(finalize("why? because"), "Why? Because");
    }

    #[test]
    fn test_finalize_newline_capitalize() {
        assert_eq!(finalize("hello.\nworld"), "Hello.\nWorld");
    }

    #[test]
    fn test_finalize_open_paren_no_space() {
        assert_eq!(finalize("test ( value )"), "Test (value)");
    }

    // --- preprocess edge cases ---

    #[test]
    fn test_preprocess_empty() {
        let opts = default_opts();
        assert_eq!(preprocess("", "en", &opts), "");
    }

    #[test]
    fn test_preprocess_whitespace_only() {
        let opts = default_opts();
        assert_eq!(preprocess("   ", "en", &opts), "");
    }

    #[test]
    fn test_dictation_newline_fr() {
        let opts = default_opts();
        let result = preprocess("bonjour à la ligne monde", "fr", &opts);
        assert!(result.contains('\n'), "Expected newline in: {:?}", result);
    }

    #[test]
    fn test_dictation_paragraph_en() {
        let opts = default_opts();
        let result = preprocess("hello new paragraph world", "en", &opts);
        assert!(result.contains("\n\n"), "Expected double newline in: {:?}", result);
    }

    // --- resolve_language ---

    #[test]
    fn test_auto_detect_english() {
        let result = process("the cat is on the table", "auto", &default_opts());
        // No French words → should default to English
        assert!(result.starts_with("The"));
    }

    // --- hallucination edge cases ---

    #[test]
    fn test_hallucination_ellipsis_unicode() {
        let opts = default_opts();
        assert_eq!(process("\u{2026}", "en", &opts), "");
    }

    #[test]
    fn test_hallucination_with_trailing_dots() {
        let opts = default_opts();
        assert_eq!(process("sous-titrage...", "fr", &opts), "");
    }

    // --- fillers with surrounding text preserved ---

    #[test]
    fn test_fillers_dont_eat_real_words() {
        let result = process("j'ai euh acheté euh du pain", "fr", &default_opts());
        assert!(result.contains("achet"), "Real words should survive: {}", result);
        assert!(result.contains("pain"), "Real words should survive: {}", result);
        assert!(!result.to_lowercase().contains("euh"), "Fillers should be removed");
    }

    // --- Multilingual hallucinations ---

    #[test]
    fn test_hallucination_german() {
        let opts = default_opts();
        assert_eq!(process("Untertitel im Auftrag des ZDF", "de", &opts), "");
        assert_eq!(process("Vielen Dank fürs Zuschauen", "de", &opts), "");
    }

    #[test]
    fn test_hallucination_spanish() {
        let opts = default_opts();
        assert_eq!(process("Gracias por ver", "es", &opts), "");
        assert_eq!(process("Suscríbete al canal", "es", &opts), "");
    }

    #[test]
    fn test_hallucination_russian() {
        let opts = default_opts();
        assert_eq!(process("Спасибо за просмотр", "ru", &opts), "");
    }

    #[test]
    fn test_hallucination_repetition() {
        let opts = default_opts();
        // Same word repeated 3+ times → hallucination loop
        assert_eq!(process("okay okay okay okay", "en", &opts), "");
        assert_eq!(process("the the the the the", "en", &opts), "");
    }

    #[test]
    fn test_hallucination_music_symbols() {
        let opts = default_opts();
        assert_eq!(process("♪ ♪ ♪", "en", &opts), "");
        assert_eq!(process("♫", "en", &opts), "");
    }

    // --- Multilingual fillers ---

    #[test]
    fn test_fillers_german() {
        let result = process("äh ich habe ähm das gemacht", "de", &default_opts());
        assert!(!result.to_lowercase().contains("äh"), "German fillers should be removed");
        assert!(result.contains("gemacht"));
    }

    #[test]
    fn test_fillers_spanish() {
        let result = process("eh pues hola mundo", "es", &default_opts());
        assert!(!result.to_lowercase().contains(" eh "));
        assert!(result.contains("ola")); // Hola capitalized
    }

    #[test]
    fn test_fillers_russian() {
        let result = process("ну вот привет мир", "ru", &default_opts());
        assert!(!result.to_lowercase().contains("ну "));
    }
}
