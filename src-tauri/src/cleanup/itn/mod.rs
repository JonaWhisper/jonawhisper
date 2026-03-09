//! Inverse Text Normalization (ITN) — converts spoken forms to written canonical forms.
//!
//! Supports 9 languages: FR, EN, DE, ES, PT, IT, NL, PL, RU.
//! Each language has: cardinal numbers, ordinals, percentages, hours, currencies, units.
//! Per-language rules live in their own submodule for easy extension.

use regex::Regex;
use std::sync::Mutex;
use std::time::SystemTime;

// Macro must be defined before `mod` declarations so child modules can use it.
macro_rules! regex_rules {
    ($name:ident, [ $(($pat:expr, $rep:expr)),* $(,)? ]) => {
        static $name: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
            vec![ $( (Regex::new($pat).unwrap(), $rep), )* ]
        });
    };
}

mod de;
mod en;
mod es;
mod fr;
mod it;
mod nl;
mod pl;
mod pt;
mod ru;

/// Extract base language code: "fr-CA" → "fr"
fn lang_base(language: &str) -> &str {
    language.split(&['-', '_'][..]).next().unwrap_or(language)
}

/// User ITN mappings (abbreviation=expansion from user_dict.txt).
struct UserItn {
    rules: Vec<(Regex, String)>,
    mtime: SystemTime,
}

static USER_ITN: Mutex<Option<UserItn>> = Mutex::new(None);

/// Load or reload user ITN mappings from user_dict.txt if the file changed.
fn refresh_user_itn() {
    let path = crate::cleanup::symspell_correct::user_dict_path();
    let Ok(meta) = std::fs::metadata(&path) else {
        return;
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };

    let mut guard = USER_ITN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ref ui) = *guard {
        if ui.mtime == mtime {
            return;
        }
    }

    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut rules = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((pattern, replacement)) = line.split_once('=') {
            let pattern = pattern.trim();
            let replacement = replacement.trim();
            if !pattern.is_empty() {
                if let Ok(re) = Regex::new(&format!("(?i)\\b{}\\b", regex::escape(pattern))) {
                    rules.push((re, replacement.to_string()));
                }
            }
        }
    }
    if !rules.is_empty() {
        log::info!("User ITN: loaded {} mappings", rules.len());
    }
    *guard = Some(UserItn { rules, mtime });
}

/// Apply user ITN mappings to text.
fn apply_user_itn(text: &str) -> String {
    let guard = USER_ITN.lock().unwrap_or_else(|e| e.into_inner());
    let Some(ref ui) = *guard else {
        return text.to_string();
    };
    let mut result = text.to_string();
    for (re, replacement) in &ui.rules {
        result = re.replace_all(&result, replacement.as_str()).to_string();
    }
    result
}

/// Apply ITN transformations to text.
pub fn apply_itn(text: &str, language: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    refresh_user_itn();

    let result = match lang_base(language) {
        "fr" => fr::apply_all(text),
        "de" => de::apply_all(text),
        "es" => es::apply_all(text),
        "pt" => pt::apply_all(text),
        "it" => it::apply_all(text),
        "nl" => nl::apply_all(text),
        "pl" => pl::apply_all(text),
        "ru" => ru::apply_all(text),
        _ => en::apply_all(text),
    };

    // Apply user-defined ITN mappings last (user overrides take precedence)
    apply_user_itn(&result)
}

/// Apply a list of (regex, replacement) pairs to text.
fn apply_regex_list(text: &str, rules: &[(Regex, &str)]) -> String {
    let mut result = text.to_string();
    for (re, replacement) in rules {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

/// Symbols that are always recognized as units (language-independent, after regex replacement).
const UNIT_SYMBOLS: &[&str] = &[
    "$", "\u{20ac}", "\u{00a3}", "%", "km", "kg", "g", "L", "mL", "m", "cm", "mm", "mi", "\u{00b0}",
    "h", "min", "s",
];

/// Check if a word is a known unit, using the language-specific list + universal symbols.
fn is_unit_word(word: &str, lang_units: &[&str]) -> bool {
    let lower = word.to_lowercase();
    lang_units.iter().any(|u| lower == *u)
        || UNIT_SYMBOLS.contains(&word)
}

/// Strip trailing punctuation from a word for number parsing.
/// ASR + punctuation models insert commas and periods that prevent number recognition
/// (e.g. "Deux," "zéro," "quatre."). The number parser's own guards prevent
/// cross-boundary combining of separate numbers.
fn strip_trailing_punct(word: &str) -> &str {
    word.trim_end_matches(|c: char| matches!(c, '.' | ',' | '!' | '?' | ';' | ':'))
}

/// Split text into words, try to parse number sequences, replace with digits.
fn replace_numbers(text: &str, parser: fn(&[&str]) -> Option<(u64, usize)>, lang_units: &[&str]) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }

    // Strip trailing sentence punctuation for number parsing
    // (ASR output often has "quatre." at end of sentence)
    let clean_words: Vec<&str> = words.iter().map(|w| strip_trailing_punct(w)).collect();

    let mut result = String::new();
    let mut i = 0;
    let mut text_pos = 0;

    while i < words.len() {
        let word_pos = text[text_pos..].find(words[i]).map(|p| text_pos + p).unwrap_or(text_pos);
        result.push_str(&text[text_pos..word_pos]);

        if let Some((value, consumed)) = parser(&clean_words[i..]) {
            // Don't convert standalone "un"/"une"/"a"/"one" — too ambiguous as article
            // UNLESS the next word is a known unit (heure, euro, kilo, etc.)
            // Note: value==0 ("zero") is never an article, always convert it.
            if consumed == 1 && value == 1 {
                let next_is_unit = clean_words.get(i + 1).is_some_and(|w| is_unit_word(w, lang_units));
                if !next_is_unit {
                    result.push_str(words[i]);
                    text_pos = word_pos + words[i].len();
                    i += 1;
                    continue;
                }
            }

            result.push_str(&value.to_string());
            // Preserve trailing punctuation from the last consumed word
            let last_word = i + consumed - 1;
            let trailing = &words[last_word][clean_words[last_word].len()..];
            result.push_str(trailing);

            text_pos = text[text_pos..].find(words[last_word])
                .map(|p| text_pos + p + words[last_word].len())
                .unwrap_or(word_pos + words[i].len());
            i += consumed;
        } else {
            result.push_str(words[i]);
            text_pos = word_pos + words[i].len();
            i += 1;
        }
    }

    if text_pos < text.len() {
        result.push_str(&text[text_pos..]);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_change_on_regular_text() {
        assert_eq!(apply_itn("bonjour le monde", "fr"), "bonjour le monde");
        assert_eq!(apply_itn("hello world", "en"), "hello world");
    }

    #[test]
    fn empty_input() {
        assert_eq!(apply_itn("", "fr"), "");
        assert_eq!(apply_itn("  ", "en"), "  ");
    }

    #[test]
    fn multiple_numbers_in_sentence() {
        assert_eq!(apply_itn("j'ai cinq chats et trois chiens", "fr"), "j'ai 5 chats et 3 chiens");
    }

    #[test]
    fn lang_prefix_routes() {
        assert_eq!(apply_itn("cinq euros", "fr-FR"), "5 \u{20AC}");
        assert_eq!(apply_itn("five dollars", "en-US"), "5 $");
    }
}
