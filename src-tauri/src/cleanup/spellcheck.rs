//! Spell-checking via spellbook (Hunspell-compatible, pure Rust).
//!
//! Bundled dictionaries: FR (LibreOffice fr.dic/fr.aff), EN (en_US.dic/en_US.aff).
//! Detects misspelled words and replaces them with the top suggestion (edit distance ≤ 2).

use std::sync::OnceLock;

static DICT_FR_AFF: &str = include_str!("../../dicts/fr.aff");
static DICT_FR_DIC: &str = include_str!("../../dicts/fr.dic");
static DICT_EN_AFF: &str = include_str!("../../dicts/en_US.aff");
static DICT_EN_DIC: &str = include_str!("../../dicts/en_US.dic");

static DICT_FR: OnceLock<Option<spellbook::Dictionary>> = OnceLock::new();
static DICT_EN: OnceLock<Option<spellbook::Dictionary>> = OnceLock::new();

fn get_dict_fr() -> Option<&'static spellbook::Dictionary> {
    DICT_FR.get_or_init(|| {
        match spellbook::Dictionary::new(DICT_FR_AFF, DICT_FR_DIC) {
            Ok(d) => {
                log::info!("Spell-check: FR dictionary loaded");
                Some(d)
            }
            Err(e) => {
                log::warn!("Spell-check: failed to load FR dictionary: {}", e);
                None
            }
        }
    }).as_ref()
}

fn get_dict_en() -> Option<&'static spellbook::Dictionary> {
    DICT_EN.get_or_init(|| {
        match spellbook::Dictionary::new(DICT_EN_AFF, DICT_EN_DIC) {
            Ok(d) => {
                log::info!("Spell-check: EN dictionary loaded");
                Some(d)
            }
            Err(e) => {
                log::warn!("Spell-check: failed to load EN dictionary: {}", e);
                None
            }
        }
    }).as_ref()
}

/// Auto-correct misspelled words using the appropriate dictionary.
/// Returns the corrected text, or the original if no dictionary is available.
pub fn auto_correct(text: &str, language: &str) -> String {
    let dict = if language.starts_with("fr") {
        get_dict_fr()
    } else {
        get_dict_en()
    };

    let dict = match dict {
        Some(d) => d,
        None => return text.to_string(),
    };

    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;

    // Iterate through words, preserving all non-word characters exactly
    for (start, word) in word_boundaries(text) {
        // Append everything between words (spaces, punctuation) as-is
        result.push_str(&text[last_end..start]);
        last_end = start + word.len();

        // Skip short words, numbers, and already-correct words
        if word.len() <= 1 || word.chars().any(|c| c.is_ascii_digit()) || dict.check(word) {
            result.push_str(word);
            continue;
        }

        // Also skip ALL-CAPS words (acronyms)
        if word.chars().all(|c| c.is_uppercase() || !c.is_alphabetic()) {
            result.push_str(word);
            continue;
        }

        // Try to get a suggestion
        let mut suggestions = Vec::new();
        dict.suggest(word, &mut suggestions);

        if let Some(suggestion) = suggestions.first() {
            // Preserve original casing pattern
            let corrected = match_case(word, suggestion);
            log::debug!("Spell-check: {} → {}", word, corrected);
            result.push_str(&corrected);
        } else {
            result.push_str(word);
        }
    }

    // Append any trailing content
    result.push_str(&text[last_end..]);
    result
}

/// Yield (byte_offset, word_str) for each word in the text.
fn word_boundaries(text: &str) -> Vec<(usize, &str)> {
    let mut words = Vec::new();
    let mut start = None;

    for (i, ch) in text.char_indices() {
        if ch.is_alphanumeric() || ch == '\'' || ch == '\u{2019}' || ch == '-' {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start {
            let word = &text[s..i];
            // Don't include trailing hyphens/apostrophes
            let trimmed = word.trim_end_matches(|c: char| c == '-' || c == '\'' || c == '\u{2019}');
            if !trimmed.is_empty() {
                words.push((s, trimmed));
            }
            start = None;
        }
    }

    if let Some(s) = start {
        let word = &text[s..];
        let trimmed = word.trim_end_matches(|c: char| c == '-' || c == '\'' || c == '\u{2019}');
        if !trimmed.is_empty() {
            words.push((s, trimmed));
        }
    }

    words
}

/// Match the casing of the original word to the suggestion.
fn match_case(original: &str, suggestion: &str) -> String {
    let orig_chars: Vec<char> = original.chars().collect();

    if orig_chars.iter().all(|c| c.is_uppercase() || !c.is_alphabetic()) {
        // ALL CAPS → keep all caps
        suggestion.to_uppercase()
    } else if orig_chars.first().map_or(false, |c| c.is_uppercase()) {
        // Title Case → capitalize first
        let mut chars = suggestion.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            None => String::new(),
        }
    } else {
        suggestion.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_case() {
        assert_eq!(match_case("Hello", "bonjour"), "Bonjour");
        assert_eq!(match_case("HELLO", "bonjour"), "BONJOUR");
        assert_eq!(match_case("hello", "bonjour"), "bonjour");
    }

    #[test]
    fn test_word_boundaries() {
        let words = word_boundaries("Hello, world! Test.");
        assert_eq!(words.len(), 3);
        assert_eq!(words[0].1, "Hello");
        assert_eq!(words[1].1, "world");
        assert_eq!(words[2].1, "Test");
    }

    #[test]
    fn test_word_boundaries_apostrophe() {
        let words = word_boundaries("l'homme aujourd'hui");
        // Should keep apostrophes within words
        assert_eq!(words[0].1, "l'homme");
        assert_eq!(words[1].1, "aujourd'hui");
    }

    #[test]
    fn test_correct_text_preserves_punctuation() {
        // Known-good words should pass through unchanged
        let result = auto_correct("Hello, world.", "en");
        assert_eq!(result, "Hello, world.");
    }
}
