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

pub fn process(text: &str, language: &str) -> String {
    let mut result = strip_hallucinations(text);
    if result.trim().is_empty() {
        return String::new();
    }

    let lang = resolve_language(language, &result);
    result = apply_dictation_commands(&result, &lang);
    result = fix_punctuation_spacing(&result);
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

/// Known Whisper hallucination phrases that appear on silence/noise.
const HALLUCINATIONS: &[&str] = &[
    "sous-titrage société radio-canada",
    "sous-titrage st",
    "sous titrage société radio canada",
    "soustitrage société radio-canada",
    "sous-titrage",
    "sous-titres par",
    "subtitles by",
    "amara.org",
    "thank you for watching",
    "thanks for watching",
    "merci d'avoir regardé",
    "merci pour votre écoute",
    "please subscribe",
    "like and subscribe",
    "www.",
    "http",
    "bye.",
    "bye bye.",
    "bye-bye.",
    "au revoir.",
    "à bientôt.",
    "♪",
    "...",
    "…",
];

/// Strip known Whisper hallucination phrases from text.
/// If only hallucinations remain, returns empty string.
fn strip_hallucinations(text: &str) -> String {
    let mut result = text.to_string();
    let lower = result.to_lowercase();

    // If the entire text (trimmed, case-insensitive) matches a hallucination, discard it
    let trimmed_lower = lower.trim().trim_matches('.').trim();
    for h in HALLUCINATIONS {
        if trimmed_lower == *h {
            log::info!("Filtered hallucination: {:?}", text.trim());
            return String::new();
        }
    }

    // Remove hallucination phrases embedded in longer text
    for h in HALLUCINATIONS {
        let re = Regex::new(&format!("(?i){}", regex::escape(h))).unwrap();
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

fn apply_dictation_commands(text: &str, language: &str) -> String {
    let commands: Vec<(&str, &str)> = if language.starts_with("fr") {
        vec![
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
    } else {
        vec![
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
    };

    let mut result = text.to_string();
    for (pattern, replacement) in commands {
        let re = Regex::new(&format!("(?i){}", regex::escape(pattern))).unwrap();
        result = re.replace_all(&result, replacement).to_string();
    }
    result
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

    #[test]
    fn test_french_commands() {
        let result = process("bonjour virgule comment allez-vous point d'interrogation", "fr");
        assert_eq!(result, "Bonjour, comment allez-vous?");
    }

    #[test]
    fn test_english_commands() {
        let result = process("hello comma how are you question mark", "en");
        assert_eq!(result, "Hello, how are you?");
    }

    #[test]
    fn test_auto_detect_french() {
        let result = process("le chat est dans la maison", "auto");
        assert!(result.starts_with("Le"));
    }

    #[test]
    fn test_capitalization() {
        let result = process("hello. world", "en");
        assert_eq!(result, "Hello. World");
    }

    #[test]
    fn test_hallucination_filter() {
        assert_eq!(process("Sous-titrage Société Radio-Canada", "fr"), "");
        assert_eq!(process("sous-titrage", "fr"), "");
        assert_eq!(process("Thank you for watching", "en"), "");
        assert_eq!(process("...", "en"), "");
    }

    #[test]
    fn test_hallucination_embedded() {
        let result = process("bonjour sous-titrage tout le monde", "fr");
        assert!(result.contains("Bonjour"));
        assert!(!result.to_lowercase().contains("sous-titrage"));
    }
}
