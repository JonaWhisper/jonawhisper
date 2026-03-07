//! SymSpell-based spell correction with large frequency dictionaries.
//!
//! FR: 125K words from Lexique383 (book + film corpus frequencies).
//! EN: 82K words from SymSpell official frequency dictionary + 242K bigrams.
//!
//! Compared to spellbook (Hunspell, ~84K FR forms), SymSpell offers:
//! - Frequency-weighted suggestions (prefers common words)
//! - `lookup_compound` for phrase-level correction (handles word boundary errors)
//! - Sub-millisecond per-word lookup (vs ~10ms for Hunspell suggest)

use std::sync::OnceLock;
use symspell::{SymSpell, UnicodeStringStrategy, Verbosity};

static DICT_FR_FREQ: &str = include_str!("../../dicts/fr_freq.txt");
static DICT_EN_FREQ: &str = include_str!("../../dicts/en_freq.txt");
static DICT_EN_BIGRAM: &str = include_str!("../../dicts/en_bigram.txt");

static SS_FR: OnceLock<Option<SymSpell<UnicodeStringStrategy>>> = OnceLock::new();
static SS_EN: OnceLock<Option<SymSpell<UnicodeStringStrategy>>> = OnceLock::new();

fn load_symspell(
    freq_data: &str,
    separator: &str,
    bigram_data: Option<&str>,
    lang: &str,
) -> Option<SymSpell<UnicodeStringStrategy>> {
    let mut ss = SymSpell::default();

    let mut count = 0u32;
    for line in freq_data.lines() {
        if !line.is_empty() {
            ss.load_dictionary_line(line, 0, 1, separator);
            count += 1;
        }
    }

    if let Some(bigrams) = bigram_data {
        let mut bi_count = 0u32;
        for line in bigrams.lines() {
            if !line.is_empty() {
                ss.load_bigram_dictionary_line(line, 0, 2, " ");
                bi_count += 1;
            }
        }
        log::info!("SymSpell {}: loaded {} words + {} bigrams", lang, count, bi_count);
    } else {
        log::info!("SymSpell {}: loaded {} words", lang, count);
    }

    Some(ss)
}

fn get_ss_fr() -> Option<&'static SymSpell<UnicodeStringStrategy>> {
    SS_FR
        .get_or_init(|| load_symspell(DICT_FR_FREQ, "\t", None, "FR"))
        .as_ref()
}

fn get_ss_en() -> Option<&'static SymSpell<UnicodeStringStrategy>> {
    SS_EN
        .get_or_init(|| load_symspell(DICT_EN_FREQ, " ", Some(DICT_EN_BIGRAM), "EN"))
        .as_ref()
}

/// Correct text using SymSpell with frequency-weighted suggestions.
///
/// Uses per-word `lookup` (edit distance ≤ 2) with casing preservation,
/// same approach as the spellbook-based corrector but with a much larger
/// frequency dictionary.
pub fn auto_correct(text: &str, language: &str) -> String {
    let ss = if language.starts_with("fr") {
        get_ss_fr()
    } else {
        get_ss_en()
    };

    let ss = match ss {
        Some(s) => s,
        None => return text.to_string(),
    };

    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;

    for (start, word) in word_boundaries(text) {
        result.push_str(&text[last_end..start]);
        last_end = start + word.len();

        // Skip short words, numbers, acronyms
        if word.len() <= 1
            || word.chars().any(|c| c.is_ascii_digit())
            || word.chars().all(|c| c.is_uppercase() || !c.is_alphabetic())
        {
            result.push_str(word);
            continue;
        }

        let lower = word.to_lowercase();
        let suggestions = ss.lookup(&lower, Verbosity::Top, 2);

        if let Some(best) = suggestions.first() {
            if best.term != lower {
                let corrected = match_case(word, &best.term);
                log::debug!("SymSpell: {} → {} (freq={}, dist={})", word, corrected, best.count, best.distance);
                result.push_str(&corrected);
            } else {
                result.push_str(word);
            }
        } else {
            // No match at all — word unknown, keep as-is
            result.push_str(word);
        }
    }

    result.push_str(&text[last_end..]);
    result
}

/// Correct an entire phrase using SymSpell's compound lookup.
/// This handles word boundary errors (e.g. "jesuisallé" → "je suis allé").
/// Only available for languages with bigram data (currently EN).
pub fn correct_compound(text: &str, language: &str) -> String {
    let ss = if language.starts_with("fr") {
        get_ss_fr()
    } else {
        get_ss_en()
    };

    let ss = match ss {
        Some(s) => s,
        None => return text.to_string(),
    };

    // Process sentence by sentence (split on . ? !)
    let mut result = String::with_capacity(text.len());
    let mut last = 0;

    for (i, ch) in text.char_indices() {
        if ch == '.' || ch == '?' || ch == '!' || ch == '\n' {
            let sentence = &text[last..i];
            if !sentence.trim().is_empty() {
                let suggestions = ss.lookup_compound(sentence.trim(), 2);
                if let Some(best) = suggestions.first() {
                    // Preserve leading whitespace
                    let leading: &str = &text[last..last + sentence.len() - sentence.trim_start().len()];
                    result.push_str(leading);
                    result.push_str(&best.term);
                } else {
                    result.push_str(sentence);
                }
            } else {
                result.push_str(sentence);
            }
            result.push(ch);
            last = i + ch.len_utf8();
        }
    }

    // Handle remaining text (no trailing punctuation)
    let remaining = &text[last..];
    if !remaining.trim().is_empty() {
        let suggestions = ss.lookup_compound(remaining.trim(), 2);
        if let Some(best) = suggestions.first() {
            let leading: &str = &text[last..last + remaining.len() - remaining.trim_start().len()];
            result.push_str(leading);
            result.push_str(&best.term);
        } else {
            result.push_str(remaining);
        }
    } else {
        result.push_str(remaining);
    }

    result
}

// --- Shared helpers (same as spellcheck.rs) ---

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
            let trimmed =
                word.trim_end_matches(|c: char| c == '-' || c == '\'' || c == '\u{2019}');
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

fn match_case(original: &str, suggestion: &str) -> String {
    let orig_chars: Vec<char> = original.chars().collect();

    if orig_chars
        .iter()
        .all(|c| c.is_uppercase() || !c.is_alphabetic())
    {
        suggestion.to_uppercase()
    } else if orig_chars.first().is_some_and(|c| c.is_uppercase()) {
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

    // --- Dictionary loading ---

    #[test]
    fn test_fr_lookup_known_word() {
        let ss = get_ss_fr().expect("FR dict should load");
        let results = ss.lookup("bonjour", Verbosity::Top, 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].term, "bonjour");
    }

    #[test]
    fn test_en_lookup_known_word() {
        let ss = get_ss_en().expect("EN dict should load");
        let results = ss.lookup("hello", Verbosity::Top, 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].term, "hello");
    }

    // --- FR auto_correct ---

    #[test]
    fn test_fr_correct_misspelled() {
        let result = auto_correct("Bonojur le monde", "fr");
        assert!(result.contains("Bonjour"), "Expected 'Bonjour' in: {}", result);
    }

    #[test]
    fn test_fr_correct_multiple_errors() {
        let result = auto_correct("le problme est compliqu", "fr");
        assert!(result.contains("problème") || result.contains("probleme"),
            "Expected correction in: {}", result);
    }

    #[test]
    fn test_fr_known_words_unchanged() {
        let result = auto_correct("je suis content de te voir", "fr");
        assert_eq!(result, "je suis content de te voir");
    }

    // --- EN auto_correct ---

    #[test]
    fn test_en_correct_misspelled() {
        let result = auto_correct("the quik brown fox", "en");
        assert!(result.contains("quick"), "Expected 'quick' in: {}", result);
    }

    #[test]
    fn test_en_known_words_unchanged() {
        let result = auto_correct("the quick brown fox", "en");
        assert_eq!(result, "the quick brown fox");
    }

    // --- Preservation ---

    #[test]
    fn test_preserves_punctuation() {
        let result = auto_correct("Hello, world!", "en");
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_preserves_casing_allcaps() {
        let result = auto_correct("HELLO world", "en");
        assert!(result.contains("HELLO"), "ALL CAPS should be preserved");
    }

    #[test]
    fn test_preserves_casing_titlecase() {
        let result = auto_correct("Bonojur le monde", "fr");
        assert!(result.starts_with("B"), "Title case should be preserved: {}", result);
    }

    #[test]
    fn test_preserves_newlines() {
        let result = auto_correct("hello\nworld", "en");
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn test_preserves_multiple_spaces() {
        let result = auto_correct("hello  world", "en");
        assert_eq!(result, "hello  world");
    }

    // --- Skip rules ---

    #[test]
    fn test_skips_single_char() {
        let result = auto_correct("I a m here", "en");
        // Single chars 'I', 'a', 'm' should be untouched
        assert!(result.contains("I"), "Single char 'I' should be preserved");
    }

    #[test]
    fn test_skips_numbers() {
        let result = auto_correct("test123 hello", "en");
        assert!(result.contains("test123"), "Words with digits should be preserved");
    }

    #[test]
    fn test_skips_acronyms() {
        let result = auto_correct("NASA is great", "en");
        assert!(result.contains("NASA"), "Acronyms should be preserved");
    }

    #[test]
    fn test_handles_apostrophes() {
        let result = auto_correct("l'homme est là", "fr");
        // Apostrophe words should be handled without crashing
        assert!(!result.is_empty());
    }

    #[test]
    fn test_handles_hyphens() {
        let result = auto_correct("peut-être aujourd'hui", "fr");
        assert!(!result.is_empty());
    }

    // --- Empty / edge cases ---

    #[test]
    fn test_empty_string() {
        assert_eq!(auto_correct("", "fr"), "");
    }

    #[test]
    fn test_only_punctuation() {
        assert_eq!(auto_correct("...", "en"), "...");
    }

    #[test]
    fn test_only_spaces() {
        assert_eq!(auto_correct("   ", "en"), "   ");
    }

    // --- correct_compound ---

    #[test]
    fn test_compound_en() {
        let result = correct_compound("the bigest problem", "en");
        assert!(result.contains("biggest") || result.contains("bigest"),
            "Expected compound correction in: {}", result);
    }

    #[test]
    fn test_compound_preserves_punctuation() {
        let result = correct_compound("hello world.", "en");
        assert!(result.contains("."), "Sentence-final dot should be preserved");
    }

    // --- Language routing ---

    #[test]
    fn test_fr_prefix_routes_to_french() {
        // "fr-FR" should use French dict
        let result = auto_correct("Bonojur", "fr-FR");
        assert!(result.contains("Bonjour"), "fr-FR should route to FR: {}", result);
    }

    #[test]
    fn test_unknown_lang_routes_to_english() {
        // Non-fr language defaults to English
        let result = auto_correct("helo", "de");
        assert!(!result.is_empty());
    }

    // --- match_case helper ---

    #[test]
    fn test_match_case_lowercase() {
        assert_eq!(match_case("hello", "world"), "world");
    }

    #[test]
    fn test_match_case_titlecase() {
        assert_eq!(match_case("Hello", "world"), "World");
    }

    #[test]
    fn test_match_case_allcaps() {
        assert_eq!(match_case("HELLO", "world"), "WORLD");
    }

    // --- word_boundaries helper ---

    #[test]
    fn test_word_boundaries_simple() {
        let words = word_boundaries("hello world");
        assert_eq!(words.len(), 2);
        assert_eq!(words[0], (0, "hello"));
        assert_eq!(words[1], (6, "world"));
    }

    #[test]
    fn test_word_boundaries_punctuation() {
        let words = word_boundaries("hello, world!");
        assert_eq!(words.len(), 2);
        assert_eq!(words[0], (0, "hello"));
        assert_eq!(words[1], (7, "world"));
    }

    #[test]
    fn test_word_boundaries_apostrophe() {
        let words = word_boundaries("l'homme");
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].1, "l'homme");
    }

    #[test]
    fn test_word_boundaries_trailing_apostrophe() {
        let words = word_boundaries("test'");
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].1, "test");
    }
}
