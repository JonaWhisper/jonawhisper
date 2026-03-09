use regex::Regex;
use std::sync::LazyLock;

mod dictation;
mod fillers;
mod hallucinations;

// Punctuation spacing regexes (compiled once)
static RE_SPACE_BEFORE_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s+([.,?!;:\u{2026})\u{00BB}"\]])"#).unwrap());
static RE_SPACE_AFTER_PUNCT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([.,?!;:\u{2026}])([^\s\n.,?!;:\u{2026})\u{00BB}"\]\d])"#).unwrap());
static RE_SPACE_AFTER_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([\(\u{00AB}"\[])\s+"#).unwrap());
static RE_CAPITALIZE_AFTER_SENTENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([.?!]\s+|\n)(\p{Ll})").unwrap());

pub struct PostProcessOptions {
    pub hallucination_filter: bool,
    pub disfluency_removal: bool,
    /// Replace spoken punctuation words ("virgule" → ",", "period" → ".").
    /// Disabled when a punctuation model is active (it handles this better).
    pub dictation_commands: bool,
}

/// Phase 1: hallucination filter + dictation command substitution.
pub fn preprocess(text: &str, language: &str, opts: &PostProcessOptions) -> String {
    let mut result = if opts.hallucination_filter {
        hallucinations::strip_hallucinations(text)
    } else {
        text.to_string()
    };
    if result.trim().is_empty() {
        return String::new();
    }

    let lang = resolve_language(language, &result);

    if opts.dictation_commands {
        result = dictation::apply_dictation_commands(&result, &lang);
    }

    if opts.disfluency_removal {
        result = fillers::strip_fillers(&result, &lang);
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
        PostProcessOptions { hallucination_filter: true, disfluency_removal: true, dictation_commands: true }
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
        assert_eq!(process("Sous-titrage Soci\u{00E9}t\u{00E9} Radio-Canada", "fr", &opts), "");
        assert_eq!(process("sous-titrage", "fr", &opts), "");
        assert_eq!(process("Thank you for watching", "en", &opts), "");
        assert_eq!(process("...", "en", &opts), "");
    }

    #[test]
    fn test_hallucination_filter_disabled() {
        let opts = PostProcessOptions { hallucination_filter: false, disfluency_removal: true, dictation_commands: true };
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
        let opts = PostProcessOptions { hallucination_filter: true, disfluency_removal: false, dictation_commands: true };
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
        let result = preprocess("bonjour \u{00E0} la ligne monde", "fr", &opts);
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
        let result = process("j'ai euh achet\u{00E9} euh du pain", "fr", &default_opts());
        assert!(result.contains("achet"), "Real words should survive: {}", result);
        assert!(result.contains("pain"), "Real words should survive: {}", result);
        assert!(!result.to_lowercase().contains("euh"), "Fillers should be removed");
    }

    // --- Multilingual hallucinations ---

    #[test]
    fn test_hallucination_german() {
        let opts = default_opts();
        assert_eq!(process("Untertitel im Auftrag des ZDF", "de", &opts), "");
        assert_eq!(process("Vielen Dank f\u{00FC}rs Zuschauen", "de", &opts), "");
    }

    #[test]
    fn test_hallucination_spanish() {
        let opts = default_opts();
        assert_eq!(process("Gracias por ver", "es", &opts), "");
        assert_eq!(process("Suscr\u{00ED}bete al canal", "es", &opts), "");
    }

    #[test]
    fn test_hallucination_russian() {
        let opts = default_opts();
        assert_eq!(process("\u{0421}\u{043F}\u{0430}\u{0441}\u{0438}\u{0431}\u{043E} \u{0437}\u{0430} \u{043F}\u{0440}\u{043E}\u{0441}\u{043C}\u{043E}\u{0442}\u{0440}", "ru", &opts), "");
    }

    #[test]
    fn test_hallucination_repetition() {
        let opts = default_opts();
        assert_eq!(process("okay okay okay okay", "en", &opts), "");
        assert_eq!(process("the the the the the", "en", &opts), "");
    }

    #[test]
    fn test_hallucination_music_symbols() {
        let opts = default_opts();
        assert_eq!(process("\u{266A} \u{266A} \u{266A}", "en", &opts), "");
        assert_eq!(process("\u{266B}", "en", &opts), "");
    }

    // --- Multilingual fillers ---

    #[test]
    fn test_fillers_german() {
        let result = process("\u{00E4}h ich habe \u{00E4}hm das gemacht", "de", &default_opts());
        assert!(!result.to_lowercase().contains("\u{00E4}h"), "German fillers should be removed");
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
        let result = process("\u{043D}\u{0443} \u{0432}\u{043E}\u{0442} \u{043F}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442} \u{043C}\u{0438}\u{0440}", "ru", &default_opts());
        assert!(!result.to_lowercase().contains("\u{043D}\u{0443} "));
    }
}
