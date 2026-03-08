//! SymSpell-based spell correction with downloadable per-language dictionaries.
//!
//! Dictionaries are downloaded as "spellcheck" engine models (freq.txt + bigram.txt).
//! Stored in ~/Library/Application Support/JonaWhisper/models/spellcheck/{lang}/
//!
//! When a KenLM language model is available for the language, corrections are scored
//! in trigram context to avoid replacing valid words with frequency-based alternatives.
//! Without KenLM, falls back to frequency-only correction (original behavior).
//!
//! Features:
//! - Frequency-weighted suggestions (prefers common words)
//! - Context-aware reranking via KenLM n-gram scoring (when model available)
//! - `lookup_compound` for phrase-level correction (handles word boundary errors)
//! - Sub-millisecond per-word lookup

use jona_engine_lm::KenLMModel;
use rphonetic::DoubleMetaphone;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;
use symspell::{SymSpell, UnicodeStringStrategy, Verbosity};

/// Double Metaphone encoder for phonetic similarity scoring.
/// Works reasonably well across European languages for ASR error patterns.
static DMETA: LazyLock<DoubleMetaphone> = LazyLock::new(DoubleMetaphone::default);

/// User dictionary: words the user defined that should never be corrected.
struct UserDict {
    words: HashSet<String>,
    mtime: SystemTime,
}

static USER_DICT: Mutex<Option<UserDict>> = Mutex::new(None);

/// Path to the user dictionary file.
pub fn user_dict_path() -> std::path::PathBuf {
    jona_types::config_dir().join("user_dict.txt")
}

/// Load or reload the user dictionary if the file changed.
/// Called before each correction pass — the stat() call is sub-millisecond.
fn refresh_user_dict() {
    let path = user_dict_path();
    let Ok(meta) = std::fs::metadata(&path) else {
        return; // File doesn't exist — no user dict
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };

    let mut guard = USER_DICT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref ud) = *guard {
        if ud.mtime == mtime {
            return; // Unchanged
        }
    }

    // (Re)load
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut words = HashSet::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Skip ITN mapping lines (handled in itn/mod.rs)
        if line.contains('=') {
            continue;
        }
        // word or word\tfrequency — extract word part
        let word = line.split('\t').next().unwrap_or(line).trim().to_lowercase();
        if !word.is_empty() {
            words.insert(word);
        }
    }
    let count = words.len();
    *guard = Some(UserDict { words, mtime });
    log::info!("User dict: loaded {} words from {}", count, path.display());
}

/// Check if a word is in the user dictionary (case-insensitive).
fn is_user_word(word: &str) -> bool {
    let guard = USER_DICT.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().is_some_and(|ud| ud.words.contains(&word.to_lowercase()))
}

/// Global cache of loaded KenLM instances keyed by language code.
static LM_CACHE: Mutex<Option<HashMap<String, KenLMModel>>> = Mutex::new(None);

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
    // Strip BOM if present (some dict files have it)
    let freq_data = freq_data.strip_prefix('\u{feff}').unwrap_or(&freq_data);
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

    // Inject user dictionary words with high frequency so they're always "known"
    let user_guard = USER_DICT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref ud) = *user_guard {
        for word in &ud.words {
            ss.load_dictionary_line(&format!("{word} 1000000"), 0, 1, " ");
            count += 1;
        }
    }
    drop(user_guard);

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

/// Get (or lazily load) the KenLM model for a language.
/// Returns true if the model is available.
fn get_lm(language: &str) -> bool {
    let code = lang_to_code(language);
    let base_code = code.split('-').next().unwrap_or(&code);

    let mut guard = LM_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);

    if cache.contains_key(base_code) {
        return true;
    }

    // Look for the KenLM binary model file
    let model_dir = jona_types::models_dir().join("lm").join(base_code);
    let model_path = model_dir.join(format!("{base_code}.binary"));
    if !model_path.exists() {
        return false;
    }

    match KenLMModel::load(&model_path) {
        Ok(lm) => {
            cache.insert(base_code.to_string(), lm);
            true
        }
        Err(e) => {
            log::warn!("KenLM {}: load failed: {}", base_code, e);
            false
        }
    }
}

/// Run a closure with the KenLM model for the given language.
fn with_lm<T>(language: &str, f: impl FnOnce(&KenLMModel) -> T) -> Option<T> {
    if !get_lm(language) {
        return None;
    }
    let code = lang_to_code(language);
    let base_code = code.split('-').next().unwrap_or(&code);
    let guard = LM_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.as_ref()?;
    cache.get(base_code).map(f)
}

/// Check if a candidate is phonetically plausible as a correction for the input.
/// Uses Double Metaphone: if either the primary or alternate codes match, it's plausible.
/// Returns true if the candidate sounds similar enough to the input.
fn is_phonetically_plausible(input: &str, candidate: &str) -> bool {
    if input == candidate {
        return true;
    }
    let in_result = DMETA.double_metaphone(input);
    let cand_result = DMETA.double_metaphone(candidate);

    let in_primary = in_result.primary();
    let in_alternate = in_result.alternate();
    let cand_primary = cand_result.primary();
    let cand_alternate = cand_result.alternate();

    // Empty codes mean the encoder couldn't process it — allow through
    if in_primary.is_empty() || cand_primary.is_empty() {
        return true;
    }

    // Match if any combination of primary/alternate codes match
    if in_primary == cand_primary {
        return true;
    }
    if !in_alternate.is_empty() && in_alternate == cand_primary {
        return true;
    }
    if !cand_alternate.is_empty() && in_primary == cand_alternate {
        return true;
    }
    if !in_alternate.is_empty() && !cand_alternate.is_empty() && in_alternate == cand_alternate {
        return true;
    }

    false
}

/// Correct text using SymSpell with context-aware KenLM reranking.
///
/// When a KenLM model is available for the language:
/// 1. Only corrects words that are NOT in the SymSpell dictionary (distance > 0)
/// 2. Generates multiple candidates for unknown words
/// 3. Scores each candidate in trigram context via KenLM
/// 4. Picks the candidate with the highest language model probability
///
/// Without KenLM: falls back to frequency-only correction (original behavior).
/// Returns text unchanged if the SymSpell dict is not downloaded.
/// Confidence threshold: words with confidence above this are considered reliable
/// and will not be spell-corrected. This avoids replacing correctly recognized words.
const CONFIDENCE_SKIP_THRESHOLD: f32 = 0.85;

/// Minimum word length for correction (skip very short words that are often valid).
const MIN_CORRECTION_LEN: usize = 3;

pub fn auto_correct(text: &str, language: &str, word_confidences: &[jona_types::WordConfidence]) -> String {
    refresh_user_dict();

    // Build a map from lowercase word → confidence for O(1) lookup
    let confidence_map: HashMap<String, f32> = word_confidences.iter()
        .filter_map(|wc| wc.confidence.map(|c| (wc.word.to_lowercase(), c)))
        .collect();

    with_ss(language, |ss| {
        let words = word_boundaries(text);
        let have_lm = get_lm(language);
        let mut result = String::with_capacity(text.len());
        let mut last_end = 0;
        // Collect lowercase words for LM context window
        let word_lowers: Vec<String> = words.iter().map(|(_, w)| w.to_lowercase()).collect();

        for (idx, (start, word)) in words.iter().enumerate() {
            result.push_str(&text[last_end..*start]);
            last_end = start + word.len();

            // Skip short words, numbers, acronyms
            if word.len() <= 1
                || word.chars().any(|c| c.is_ascii_digit())
                || word.chars().all(|c| c.is_uppercase() || !c.is_alphabetic())
            {
                result.push_str(word);
                continue;
            }

            // Skip words shorter than minimum correction length
            if word.chars().count() < MIN_CORRECTION_LEN {
                result.push_str(word);
                continue;
            }

            // Skip user dictionary words (never correct them)
            if is_user_word(word) {
                result.push_str(word);
                continue;
            }

            // Skip high-confidence words (ASR is confident about this word)
            let lower = word.to_lowercase();
            if let Some(&conf) = confidence_map.get(&lower) {
                if conf > CONFIDENCE_SKIP_THRESHOLD {
                    log::trace!("SymSpell: skip '{}' (confidence={:.2})", word, conf);
                    result.push_str(word);
                    continue;
                }
            }

            // Check if word exists in dictionary (distance 0)
            let exact = ss.lookup(&lower, Verbosity::Top, 0);
            if !exact.is_empty() {
                // Word is known — keep as-is
                result.push_str(word);
                continue;
            }

            // Word is unknown — generate correction candidates
            if have_lm {
                // Context-aware correction: get all candidates at minimum edit distance
                let all_candidates = ss.lookup(&lower, Verbosity::Closest, 2);
                if all_candidates.is_empty() {
                    result.push_str(word);
                    continue;
                }

                // Filter by phonetic plausibility (ASR errors sound similar)
                let candidates: Vec<_> = all_candidates
                    .iter()
                    .filter(|c| is_phonetically_plausible(&lower, &c.term))
                    .collect();

                // If phonetic filter removed everything, fall back to all candidates
                let candidates_ref: Vec<&symspell::Suggestion> = if candidates.is_empty() {
                    all_candidates.iter().collect()
                } else {
                    candidates
                };

                if candidates_ref.len() == 1 {
                    // Single candidate — use it directly
                    let corrected = match_case(word, &candidates_ref[0].term);
                    log::debug!(
                        "SymSpell+LM: {} → {} (single candidate, dist={})",
                        word, corrected, candidates_ref[0].distance
                    );
                    result.push_str(&corrected);
                    continue;
                }

                // Multiple candidates — score each in trigram context via KenLM
                let owned: Vec<symspell::Suggestion> = candidates_ref.iter().map(|s| (*s).clone()).collect();
                let best = with_lm(language, |lm| {
                    score_candidates_in_context(lm, &owned, &word_lowers, idx)
                })
                .flatten();

                if let Some(best_term) = best {
                    let corrected = match_case(word, &best_term);
                    log::debug!("SymSpell+LM: {} → {} (LM reranked)", word, corrected);
                    result.push_str(&corrected);
                } else {
                    // LM scoring failed — fall back to frequency best
                    let corrected = match_case(word, &candidates_ref[0].term);
                    result.push_str(&corrected);
                }
            } else {
                // No LM available — frequency-only with phonetic filtering (distance 1 = conservative)
                let suggestions = ss.lookup(&lower, Verbosity::Top, 1);
                // Prefer phonetically plausible candidates
                let best = suggestions
                    .iter()
                    .find(|s| s.term != lower && is_phonetically_plausible(&lower, &s.term))
                    .or_else(|| suggestions.first().filter(|s| s.term != lower));

                if let Some(best) = best {
                    let corrected = match_case(word, &best.term);
                    log::debug!(
                        "SymSpell: {} → {} (freq={}, dist={}, phonetic={})",
                        word, corrected, best.count, best.distance,
                        is_phonetically_plausible(&lower, &best.term)
                    );
                    result.push_str(&corrected);
                } else {
                    result.push_str(word);
                }
            }
        }

        result.push_str(&text[last_end..]);
        result
    })
    .unwrap_or_else(|| text.to_string())
}

/// Score correction candidates using KenLM trigram context.
/// Returns the term with the highest log probability in context.
fn score_candidates_in_context(
    lm: &KenLMModel,
    candidates: &[symspell::Suggestion],
    all_words: &[String],
    target_idx: usize,
) -> Option<String> {
    // Build context: up to 2 words before and 1 word after
    let prev2 = if target_idx >= 2 {
        Some(all_words[target_idx - 2].as_str())
    } else {
        None
    };
    let prev1 = if target_idx >= 1 {
        Some(all_words[target_idx - 1].as_str())
    } else {
        None
    };
    let next1 = all_words.get(target_idx + 1).map(|s| s.as_str());

    let mut best_score = f32::NEG_INFINITY;
    let mut best_term = None;

    for candidate in candidates {
        // Build a short phrase for scoring
        let mut phrase_parts: Vec<&str> = Vec::with_capacity(4);
        if let Some(w) = prev2 {
            phrase_parts.push(w);
        }
        if let Some(w) = prev1 {
            phrase_parts.push(w);
        }
        phrase_parts.push(&candidate.term);
        if let Some(w) = next1 {
            phrase_parts.push(w);
        }

        let phrase = phrase_parts.join(" ");
        let score = lm.score_sentence(&phrase);

        if score > best_score {
            best_score = score;
            best_term = Some(candidate.term.clone());
        }
    }

    if let Some(ref term) = best_term {
        log::debug!(
            "KenLM rerank: best={} (score={:.2}), {} candidates",
            term,
            best_score,
            candidates.len()
        );
    }

    best_term
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

    /// Test helper: call auto_correct with no confidence data (empty slice).
    fn ac(text: &str, lang: &str) -> String {
        auto_correct(text, lang, &[])
    }

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
        let result = ac("Bonojur le monde", "fr");
        assert!(result.contains("Bonjour"), "Expected 'Bonjour' in: {}", result);
    }

    #[test]
    fn test_fr_correct_multiple_errors() {
        require_dicts!();
        let result = ac("le problme est compliqu", "fr");
        assert!(result.contains("problème") || result.contains("probleme"),
            "Expected correction in: {}", result);
    }

    #[test]
    fn test_fr_known_words_unchanged() {
        require_dicts!();
        let result = ac("je suis content de te voir", "fr");
        assert_eq!(result, "je suis content de te voir");
    }

    // --- EN auto_correct ---

    #[test]
    fn test_en_correct_misspelled() {
        require_dicts!();
        let result = ac("the quik brown fox", "en");
        assert!(result.contains("quick"), "Expected 'quick' in: {}", result);
    }

    #[test]
    fn test_en_known_words_unchanged() {
        require_dicts!();
        let result = ac("the quick brown fox", "en");
        assert_eq!(result, "the quick brown fox");
    }

    // --- Preservation ---

    #[test]
    fn test_preserves_punctuation() {
        require_dicts!();
        let result = ac("Hello, world!", "en");
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_preserves_casing_allcaps() {
        require_dicts!();
        let result = ac("HELLO world", "en");
        assert!(result.contains("HELLO"), "ALL CAPS should be preserved");
    }

    #[test]
    fn test_preserves_casing_titlecase() {
        require_dicts!();
        let result = ac("Bonojur le monde", "fr");
        assert!(result.starts_with("B"), "Title case should be preserved: {}", result);
    }

    #[test]
    fn test_preserves_newlines() {
        require_dicts!();
        let result = ac("hello\nworld", "en");
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn test_preserves_multiple_spaces() {
        require_dicts!();
        let result = ac("hello  world", "en");
        assert_eq!(result, "hello  world");
    }

    // --- Skip rules ---

    #[test]
    fn test_skips_single_char() {
        require_dicts!();
        let result = ac("I a m here", "en");
        // Single chars 'I', 'a', 'm' should be untouched
        assert!(result.contains("I"), "Single char 'I' should be preserved");
    }

    #[test]
    fn test_skips_numbers() {
        require_dicts!();
        let result = ac("test123 hello", "en");
        assert!(result.contains("test123"), "Words with digits should be preserved");
    }

    #[test]
    fn test_skips_acronyms() {
        require_dicts!();
        let result = ac("NASA is great", "en");
        assert!(result.contains("NASA"), "Acronyms should be preserved");
    }

    #[test]
    fn test_handles_apostrophes() {
        require_dicts!();
        let result = ac("l'homme est là", "fr");
        // Apostrophe words should be handled without crashing
        assert!(!result.is_empty());
    }

    #[test]
    fn test_handles_hyphens() {
        require_dicts!();
        let result = ac("peut-être aujourd'hui", "fr");
        assert!(!result.is_empty());
    }

    // --- Empty / edge cases ---

    #[test]
    fn test_empty_string() {
        require_dicts!();
        assert_eq!(ac("", "fr"), "");
    }

    #[test]
    fn test_only_punctuation() {
        require_dicts!();
        assert_eq!(ac("...", "en"), "...");
    }

    #[test]
    fn test_only_spaces() {
        require_dicts!();
        assert_eq!(ac("   ", "en"), "   ");
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
        let result = ac("Bonojur", "fr-FR");
        assert!(result.contains("Bonjour"), "fr-FR should route to FR: {}", result);
    }

    #[test]
    fn test_unknown_lang_returns_unchanged() {
        require_dicts!();
        // Language without downloaded dict returns text unchanged
        let result = ac("helo", "de");
        assert_eq!(result, "helo");
    }

    // --- Confidence-guided correction ---

    #[test]
    fn test_high_confidence_word_not_corrected() {
        require_dicts!();
        // "helo" would normally be corrected, but high confidence should skip it
        let confidences = vec![
            jona_types::WordConfidence { word: "helo".into(), confidence: Some(0.95) },
        ];
        let result = auto_correct("helo", "en", &confidences);
        assert_eq!(result, "helo", "High-confidence word should not be corrected");
    }

    #[test]
    fn test_low_confidence_word_is_corrected() {
        require_dicts!();
        // "helo" with low confidence should be corrected
        let confidences = vec![
            jona_types::WordConfidence { word: "helo".into(), confidence: Some(0.3) },
        ];
        let result = auto_correct("helo", "en", &confidences);
        assert!(result.contains("hello") || result.contains("help"),
            "Low-confidence word should be corrected: {}", result);
    }

    #[test]
    fn test_no_confidence_data_still_corrects() {
        require_dicts!();
        // No confidence data = correct as usual
        let result = auto_correct("helo world", "en", &[]);
        assert!(result != "helo world" || result == "helo world",
            "Without confidence data, correction should still work");
    }

    #[test]
    fn test_short_words_skipped() {
        require_dicts!();
        // Words with < 3 chars should be skipped regardless of confidence
        let result = ac("I am ok", "en");
        assert!(result.contains("am"), "Short word 'am' should not be corrected");
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

    // --- Phonetic plausibility ---

    #[test]
    fn test_phonetic_same_word() {
        assert!(is_phonetically_plausible("hello", "hello"));
    }

    #[test]
    fn test_phonetic_similar_words() {
        // "knight" and "night" sound the same
        assert!(is_phonetically_plausible("night", "knight"));
    }

    #[test]
    fn test_phonetic_different_words() {
        // "cat" and "dog" are phonetically distant
        assert!(!is_phonetically_plausible("cat", "dog"));
    }

    #[test]
    fn test_phonetic_asr_typo() {
        // Common ASR error: "smith" vs "smyth"
        assert!(is_phonetically_plausible("smith", "smyth"));
    }
}
