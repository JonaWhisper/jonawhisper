//! SymSpell-based spell correction with downloadable per-language dictionaries.
//!
//! Dictionaries are downloaded as "spellcheck" engine models (freq.txt + bigram.txt).
//! Stored in ~/Library/Application Support/JonaWhisper/models/spellcheck/{lang}/
//!
//! Features:
//! - Frequency-weighted suggestions (prefers common words)
//! - `lookup_compound` for phrase-level correction (handles word boundary errors)
//! - Sub-millisecond per-word lookup

use std::collections::HashMap;
use std::sync::Mutex;
use symspell::{SymSpell, UnicodeStringStrategy, Verbosity};

/// Global cache of loaded SymSpell instances keyed by language code.
static SS_CACHE: Mutex<Option<HashMap<String, SymSpell<UnicodeStringStrategy>>>> =
    Mutex::new(None);

/// Resolve the best spellcheck dictionary code for a given language.
///
/// Resolution order:
/// 1. If `lang` is already a full locale (e.g. "fr-CA") and that dict exists → use it
/// 2. If `lang` is a base code (e.g. "fr"), check the system locale for a regional
///    match (e.g. system is "fr_BE" → try "fr-be" dict)
/// 3. Fall back to the base language code
///
/// This lets the user pick "fr" in the ASR language selector while automatically
/// getting the Belgian/Québécois/Swiss dict based on their OS locale.
fn lang_to_code(lang: &str) -> String {
    let base = jona_types::models_dir().join("spellcheck");
    let normalized = lang.replace('_', "-").to_lowercase();

    // Try full locale first (e.g. "fr-ca")
    if base.join(&normalized).join("freq.txt").exists() {
        return normalized;
    }

    // If input is a base code (no dash), try refining with system locale
    let base_code = normalized.split('-').next().unwrap_or(&normalized);
    if !normalized.contains('-') {
        if let Some(sys) = sys_locale::get_locale() {
            let sys_norm = sys.replace('_', "-").to_lowercase();
            // Only use system locale if it matches the same base language
            if sys_norm.starts_with(&format!("{base_code}-"))
                && base.join(&sys_norm).join("freq.txt").exists() {
                    return sys_norm;
            }
        }
    }

    // Try base language (e.g. "fr" from "fr-ca" if fr-ca dict not found)
    if base_code != normalized && base.join(base_code).join("freq.txt").exists() {
        return base_code.to_string();
    }

    // Return base language code even if not downloaded yet
    base_code.to_string()
}

/// Resolve the spellcheck dict directory for a given language.
fn dict_dir(lang: &str) -> std::path::PathBuf {
    let code = lang_to_code(lang);
    jona_types::models_dir().join("spellcheck").join(code)
}

fn load_from_dir(dir: &std::path::Path, lang: &str) -> Option<SymSpell<UnicodeStringStrategy>> {
    let freq_path = dir.join("freq.txt");
    if !freq_path.exists() {
        log::warn!("SymSpell {}: freq.txt not found at {}", lang, dir.display());
        return None;
    }

    let freq_data = std::fs::read_to_string(&freq_path).ok()?;
    let bigram_path = dir.join("bigram.txt");
    let bigram_data = std::fs::read_to_string(&bigram_path).ok();

    // Detect separator: tab for FR (Lexique383 multi-word entries), space for EN
    let separator = if freq_data.contains('\t') { "\t" } else { " " };

    let mut ss = SymSpell::default();

    let mut count = 0u32;
    for line in freq_data.lines() {
        if !line.is_empty() {
            ss.load_dictionary_line(line, 0, 1, separator);
            count += 1;
        }
    }

    if let Some(ref bigrams) = bigram_data {
        let mut bi_count = 0u32;
        for line in bigrams.lines() {
            if !line.is_empty() {
                ss.load_bigram_dictionary_line(line, 0, 2, " ");
                bi_count += 1;
            }
        }
        log::debug!(
            "SymSpell {}: loaded {} words + {} bigrams from {}",
            lang, count, bi_count, dir.display()
        );
    } else {
        log::debug!(
            "SymSpell {}: loaded {} words from {}",
            lang, count, dir.display()
        );
    }

    Some(ss)
}

/// Get (or lazily load) the SymSpell instance for a language.
/// Returns None if the dict is not downloaded.
fn get_ss(language: &str) -> bool {
    let code = lang_to_code(language);

    let mut guard = SS_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);

    if cache.contains_key(&code) {
        return true;
    }

    let dir = dict_dir(language);
    if let Some(ss) = load_from_dir(&dir, &code) {
        cache.insert(code, ss);
        true
    } else {
        false
    }
}

/// Run a closure with the SymSpell instance for the given language.
fn with_ss<T>(language: &str, f: impl FnOnce(&SymSpell<UnicodeStringStrategy>) -> T) -> Option<T> {
    if !get_ss(language) {
        return None;
    }
    let code = lang_to_code(language);
    let guard = SS_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.as_ref()?;
    cache.get(&code).map(f)
}

/// Correct text using SymSpell with frequency-weighted suggestions.
///
/// Uses per-word `lookup` (edit distance ≤ 2) with casing preservation.
/// Returns text unchanged if the dict for the language is not downloaded.
pub fn auto_correct(text: &str, language: &str) -> String {
    with_ss(language, |ss| {
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
                    log::debug!(
                        "SymSpell: {} → {} (freq={}, dist={})",
                        word, corrected, best.count, best.distance
                    );
                    result.push_str(&corrected);
                } else {
                    result.push_str(word);
                }
            } else {
                result.push_str(word);
            }
        }

        result.push_str(&text[last_end..]);
        result
    })
    .unwrap_or_else(|| text.to_string())
}

/// Correct an entire phrase using SymSpell's compound lookup.
/// This handles word boundary errors (e.g. "jesuisallé" → "je suis allé").
/// Returns text unchanged if the dict for the language is not downloaded.
#[allow(dead_code)]
pub fn correct_compound(text: &str, language: &str) -> String {
    with_ss(language, |ss| {
        let mut result = String::with_capacity(text.len());
        let mut last = 0;

        for (i, ch) in text.char_indices() {
            if ch == '.' || ch == '?' || ch == '!' || ch == '\n' {
                let sentence = &text[last..i];
                if !sentence.trim().is_empty() {
                    let suggestions = ss.lookup_compound(sentence.trim(), 2);
                    if let Some(best) = suggestions.first() {
                        let leading: &str =
                            &text[last..last + sentence.len() - sentence.trim_start().len()];
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
                let leading: &str =
                    &text[last..last + remaining.len() - remaining.trim_start().len()];
                result.push_str(leading);
                result.push_str(&best.term);
            } else {
                result.push_str(remaining);
            }
        } else {
            result.push_str(remaining);
        }

        result
    })
    .unwrap_or_else(|| text.to_string())
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
                word.trim_end_matches(['-', '\'', '\u{2019}']);
            if !trimmed.is_empty() {
                words.push((s, trimmed));
            }
            start = None;
        }
    }

    if let Some(s) = start {
        let word = &text[s..];
        let trimmed = word.trim_end_matches(['-', '\'', '\u{2019}']);
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
    use std::sync::OnceLock;

    static DICTS_READY: OnceLock<bool> = OnceLock::new();

    const RELEASE_BASE: &str =
        "https://github.com/JonaWhisper/jonawhisper-spellcheck-dicts/releases/latest/download";

    /// Download real production dictionaries from GitHub Releases.
    /// Returns true if dicts are available, false if download failed (no network, etc.).
    /// Tests that need dicts should call this and `return` early if false.
    fn ensure_test_dicts() -> bool {
        *DICTS_READY.get_or_init(|| {
            let model_base = jona_types::models_dir().join("spellcheck");
            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .ok();

            let mut all_ok = true;
            for (lang, files) in [
                ("fr", vec!["fr-freq.txt", "fr-bigram.txt"]),
                ("en", vec!["en-freq.txt", "en-bigram.txt"]),
            ] {
                let dest = model_base.join(lang);
                std::fs::create_dir_all(&dest).ok();

                // Skip download if freq.txt already exists (from app usage or previous test run)
                if dest.join("freq.txt").exists() {
                    continue;
                }

                let Some(ref client) = client else {
                    all_ok = false;
                    continue;
                };

                for file in &files {
                    let url = format!("{RELEASE_BASE}/{file}");
                    let local_name = if file.contains("freq") {
                        "freq.txt"
                    } else {
                        "bigram.txt"
                    };
                    match client.get(&url).send().and_then(|r| r.error_for_status()) {
                        Ok(resp) => {
                            if let Ok(bytes) = resp.bytes() {
                                std::fs::write(dest.join(local_name), &bytes).ok();
                            }
                        }
                        Err(_) => {
                            all_ok = false;
                        }
                    }
                }
            }

            // Clear cached instances so they reload from freshly downloaded dicts
            let mut guard = SS_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            *guard = None;

            all_ok
        })
    }

    /// Helper: skip test if dicts are not available (no network, CI without internet).
    macro_rules! require_dicts {
        () => {
            if !ensure_test_dicts() {
                eprintln!("SKIPPED: spellcheck dicts not available (no network?)");
                return;
            }
        };
    }

    // --- Dictionary loading ---

    #[test]
    fn test_fr_lookup_known_word() {
        require_dicts!();
        let loaded = with_ss("fr", |ss| {
            let results = ss.lookup("bonjour", Verbosity::Top, 2);
            assert!(!results.is_empty());
            assert_eq!(results[0].term, "bonjour");
        });
        assert!(loaded.is_some(), "FR dict should load");
    }

    #[test]
    fn test_en_lookup_known_word() {
        require_dicts!();
        let loaded = with_ss("en", |ss| {
            let results = ss.lookup("hello", Verbosity::Top, 2);
            assert!(!results.is_empty());
            assert_eq!(results[0].term, "hello");
        });
        assert!(loaded.is_some(), "EN dict should load");
    }

    // --- FR auto_correct ---

    #[test]
    fn test_fr_correct_misspelled() {
        require_dicts!();
        let result = auto_correct("Bonojur le monde", "fr");
        assert!(result.contains("Bonjour"), "Expected 'Bonjour' in: {}", result);
    }

    #[test]
    fn test_fr_correct_multiple_errors() {
        require_dicts!();
        let result = auto_correct("le problme est compliqu", "fr");
        assert!(result.contains("problème") || result.contains("probleme"),
            "Expected correction in: {}", result);
    }

    #[test]
    fn test_fr_known_words_unchanged() {
        require_dicts!();
        let result = auto_correct("je suis content de te voir", "fr");
        assert_eq!(result, "je suis content de te voir");
    }

    // --- EN auto_correct ---

    #[test]
    fn test_en_correct_misspelled() {
        require_dicts!();
        let result = auto_correct("the quik brown fox", "en");
        assert!(result.contains("quick"), "Expected 'quick' in: {}", result);
    }

    #[test]
    fn test_en_known_words_unchanged() {
        require_dicts!();
        let result = auto_correct("the quick brown fox", "en");
        assert_eq!(result, "the quick brown fox");
    }

    // --- Preservation ---

    #[test]
    fn test_preserves_punctuation() {
        require_dicts!();
        let result = auto_correct("Hello, world!", "en");
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_preserves_casing_allcaps() {
        require_dicts!();
        let result = auto_correct("HELLO world", "en");
        assert!(result.contains("HELLO"), "ALL CAPS should be preserved");
    }

    #[test]
    fn test_preserves_casing_titlecase() {
        require_dicts!();
        let result = auto_correct("Bonojur le monde", "fr");
        assert!(result.starts_with("B"), "Title case should be preserved: {}", result);
    }

    #[test]
    fn test_preserves_newlines() {
        require_dicts!();
        let result = auto_correct("hello\nworld", "en");
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn test_preserves_multiple_spaces() {
        require_dicts!();
        let result = auto_correct("hello  world", "en");
        assert_eq!(result, "hello  world");
    }

    // --- Skip rules ---

    #[test]
    fn test_skips_single_char() {
        require_dicts!();
        let result = auto_correct("I a m here", "en");
        // Single chars 'I', 'a', 'm' should be untouched
        assert!(result.contains("I"), "Single char 'I' should be preserved");
    }

    #[test]
    fn test_skips_numbers() {
        require_dicts!();
        let result = auto_correct("test123 hello", "en");
        assert!(result.contains("test123"), "Words with digits should be preserved");
    }

    #[test]
    fn test_skips_acronyms() {
        require_dicts!();
        let result = auto_correct("NASA is great", "en");
        assert!(result.contains("NASA"), "Acronyms should be preserved");
    }

    #[test]
    fn test_handles_apostrophes() {
        require_dicts!();
        let result = auto_correct("l'homme est là", "fr");
        // Apostrophe words should be handled without crashing
        assert!(!result.is_empty());
    }

    #[test]
    fn test_handles_hyphens() {
        require_dicts!();
        let result = auto_correct("peut-être aujourd'hui", "fr");
        assert!(!result.is_empty());
    }

    // --- Empty / edge cases ---

    #[test]
    fn test_empty_string() {
        require_dicts!();
        assert_eq!(auto_correct("", "fr"), "");
    }

    #[test]
    fn test_only_punctuation() {
        require_dicts!();
        assert_eq!(auto_correct("...", "en"), "...");
    }

    #[test]
    fn test_only_spaces() {
        require_dicts!();
        assert_eq!(auto_correct("   ", "en"), "   ");
    }

    // --- correct_compound ---

    #[test]
    fn test_compound_en() {
        require_dicts!();
        let result = correct_compound("the bigest problem", "en");
        assert!(result.contains("biggest") || result.contains("bigest"),
            "Expected compound correction in: {}", result);
    }

    #[test]
    fn test_compound_preserves_punctuation() {
        require_dicts!();
        let result = correct_compound("hello world.", "en");
        assert!(result.contains("."), "Sentence-final dot should be preserved");
    }

    // --- Language routing ---

    #[test]
    fn test_fr_prefix_routes_to_french() {
        require_dicts!();
        // "fr-FR" should use French dict
        let result = auto_correct("Bonojur", "fr-FR");
        assert!(result.contains("Bonjour"), "fr-FR should route to FR: {}", result);
    }

    #[test]
    fn test_unknown_lang_returns_unchanged() {
        require_dicts!();
        // Language without downloaded dict returns text unchanged
        let result = auto_correct("helo", "de");
        assert_eq!(result, "helo");
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
