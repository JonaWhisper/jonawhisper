//! Inverse Text Normalization (ITN) — converts spoken forms to written canonical forms.
//!
//! Supports 9 languages: FR, EN, DE, ES, PT, IT, NL, PL, RU.
//! Each language has: cardinal numbers, ordinals, percentages, hours, currencies, units.
//! Per-language rules live in their own submodule for easy extension.

use regex::Regex;

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

/// Apply ITN transformations to text.
pub fn apply_itn(text: &str, language: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    match lang_base(language) {
        "fr" => fr::apply_all(text),
        "de" => de::apply_all(text),
        "es" => es::apply_all(text),
        "pt" => pt::apply_all(text),
        "it" => it::apply_all(text),
        "nl" => nl::apply_all(text),
        "pl" => pl::apply_all(text),
        "ru" => ru::apply_all(text),
        _ => en::apply_all(text),
    }
}

/// Apply a list of (regex, replacement) pairs to text.
fn apply_regex_list(text: &str, rules: &[(Regex, &str)]) -> String {
    let mut result = text.to_string();
    for (re, replacement) in rules {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

/// Split text into words, try to parse number sequences, replace with digits.
fn replace_numbers(text: &str, parser: fn(&[&str]) -> Option<(u64, usize)>) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }

    let mut result = String::new();
    let mut i = 0;
    let mut text_pos = 0;

    while i < words.len() {
        let word_pos = text[text_pos..].find(words[i]).map(|p| text_pos + p).unwrap_or(text_pos);
        result.push_str(&text[text_pos..word_pos]);

        if let Some((value, consumed)) = parser(&words[i..]) {
            // Don't convert standalone "un"/"une"/"a"/"one" — too ambiguous
            if consumed == 1 && value <= 1 {
                result.push_str(words[i]);
                text_pos = word_pos + words[i].len();
                i += 1;
                continue;
            }

            result.push_str(&value.to_string());

            let last_word = i + consumed - 1;
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
