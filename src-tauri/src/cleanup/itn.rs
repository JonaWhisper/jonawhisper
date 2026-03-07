//! Inverse Text Normalization (ITN) — converts spoken forms to written canonical forms.
//!
//! Supports FR and EN: cardinal numbers, ordinals, percentages, hours, currencies, units.

use regex::Regex;
use std::sync::LazyLock;

/// Apply ITN transformations to text.
pub fn apply_itn(text: &str, language: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    let lang = if language.starts_with("fr") { "fr" } else { "en" };
    let mut result = text.to_string();

    if lang == "fr" {
        result = apply_percentages_fr(&result);
        result = apply_hours_fr(&result);
        result = apply_currencies_fr(&result);
        result = apply_ordinals_fr(&result);
        result = apply_units_fr(&result);
        result = apply_cardinals_fr(&result);
    } else {
        result = apply_percentages_en(&result);
        result = apply_hours_en(&result);
        result = apply_currencies_en(&result);
        result = apply_ordinals_en(&result);
        result = apply_units_en(&result);
        result = apply_cardinals_en(&result);
    }

    result
}

// ---------------------------------------------------------------------------
// French number parser
// ---------------------------------------------------------------------------

/// Word-to-value mapping for French atoms.
fn fr_atom(word: &str) -> Option<u64> {
    match word {
        "zéro" | "zero" => Some(0),
        "un" | "une" => Some(1),
        "deux" => Some(2),
        "trois" => Some(3),
        "quatre" => Some(4),
        "cinq" => Some(5),
        "six" => Some(6),
        "sept" => Some(7),
        "huit" => Some(8),
        "neuf" => Some(9),
        "dix" => Some(10),
        "onze" => Some(11),
        "douze" => Some(12),
        "treize" => Some(13),
        "quatorze" => Some(14),
        "quinze" => Some(15),
        "seize" => Some(16),
        "vingt" | "vingts" => Some(20),
        "trente" => Some(30),
        "quarante" => Some(40),
        "cinquante" => Some(50),
        "soixante" => Some(60),
        _ => None,
    }
}

fn fr_multiplier(word: &str) -> Option<u64> {
    match word {
        "cent" | "cents" => Some(100),
        "mille" => Some(1_000),
        "million" | "millions" => Some(1_000_000),
        "milliard" | "milliards" => Some(1_000_000_000),
        _ => None,
    }
}

/// Parse a sequence of French number words into a number.
/// Returns (value, number_of_words_consumed).
fn parse_fr_number(words: &[&str]) -> Option<(u64, usize)> {
    if words.is_empty() {
        return None;
    }

    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase().replace('\u{2019}', "'")).collect();
    let mut pos = 0;
    let mut total: u64 = 0;
    let mut current_group: u64 = 0;
    let mut consumed_any = false;

    while pos < lower.len() {
        let w = lower[pos].as_str();

        // Skip "et" between number words (e.g. "vingt et un")
        if w == "et" {
            if consumed_any && pos + 1 < lower.len() {
                pos += 1;
                continue;
            }
            break;
        }

        // Handle "quatre-vingt(s)" — might be hyphenated or separate words
        if w == "quatre" && pos + 1 < lower.len() && (lower[pos + 1] == "vingt" || lower[pos + 1] == "vingts") {
            current_group += 80;
            pos += 2;
            consumed_any = true;
            continue;
        }

        // Handle hyphenated compound: "dix-sept", "quatre-vingt-dix-sept", etc.
        if w.contains('-') {
            let parts: Vec<&str> = w.split('-').collect();
            if let Some((val, _)) = parse_fr_number(&parts) {
                current_group += val;
                pos += 1;
                consumed_any = true;
                continue;
            }
        }

        // Atom (0-60)
        if let Some(val) = fr_atom(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }

        // Multiplier: cent, mille, million, milliard
        if let Some(mult) = fr_multiplier(w) {
            consumed_any = true;
            if mult >= 1_000_000 {
                // "trois millions" — current_group is the coefficient
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * mult;
                current_group = 0;
            } else if mult == 1_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * 1_000;
                current_group = 0;
            } else {
                // cent
                let coef = if current_group == 0 { 1 } else { current_group };
                current_group = coef * 100;
            }
            pos += 1;
            continue;
        }

        break;
    }

    if !consumed_any {
        return None;
    }

    total += current_group;
    Some((total, pos))
}

/// Replace French number words with digits in text.
fn apply_cardinals_fr(text: &str) -> String {
    replace_numbers(text, parse_fr_number)
}

// ---------------------------------------------------------------------------
// English number parser
// ---------------------------------------------------------------------------

fn en_atom(word: &str) -> Option<u64> {
    match word {
        "zero" => Some(0),
        "one" | "a" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        "eleven" => Some(11),
        "twelve" => Some(12),
        "thirteen" => Some(13),
        "fourteen" => Some(14),
        "fifteen" => Some(15),
        "sixteen" => Some(16),
        "seventeen" => Some(17),
        "eighteen" => Some(18),
        "nineteen" => Some(19),
        "twenty" => Some(20),
        "thirty" => Some(30),
        "forty" => Some(40),
        "fifty" => Some(50),
        "sixty" => Some(60),
        "seventy" => Some(70),
        "eighty" => Some(80),
        "ninety" => Some(90),
        _ => None,
    }
}

fn en_multiplier(word: &str) -> Option<u64> {
    match word {
        "hundred" => Some(100),
        "thousand" => Some(1_000),
        "million" => Some(1_000_000),
        "billion" => Some(1_000_000_000),
        _ => None,
    }
}

fn parse_en_number(words: &[&str]) -> Option<(u64, usize)> {
    if words.is_empty() {
        return None;
    }

    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let mut pos = 0;
    let mut total: u64 = 0;
    let mut current_group: u64 = 0;
    let mut consumed_any = false;

    while pos < lower.len() {
        let w = lower[pos].as_str();

        // Skip "and" between number words
        if w == "and" {
            if consumed_any && pos + 1 < lower.len() {
                pos += 1;
                continue;
            }
            break;
        }

        // Handle hyphenated: "twenty-three"
        if w.contains('-') {
            let parts: Vec<&str> = w.split('-').collect();
            if let Some((val, _)) = parse_en_number(&parts) {
                current_group += val;
                pos += 1;
                consumed_any = true;
                continue;
            }
        }

        // "a" as "one" only before multipliers
        if w == "a" && pos + 1 < lower.len() && en_multiplier(lower[pos + 1].as_str()).is_some() {
            current_group += 1;
            pos += 1;
            consumed_any = true;
            continue;
        }

        if let Some(val) = en_atom(w) {
            // Don't treat "a" as 1 when it's just the article
            if w == "a" {
                break;
            }
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }

        if let Some(mult) = en_multiplier(w) {
            consumed_any = true;
            if mult >= 1_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * mult;
                current_group = 0;
            } else {
                // hundred
                let coef = if current_group == 0 { 1 } else { current_group };
                current_group = coef * 100;
            }
            pos += 1;
            continue;
        }

        break;
    }

    if !consumed_any {
        return None;
    }

    total += current_group;
    Some((total, pos))
}

fn apply_cardinals_en(text: &str) -> String {
    replace_numbers(text, parse_en_number)
}

// ---------------------------------------------------------------------------
// Generic number replacement
// ---------------------------------------------------------------------------

/// Split text into words preserving separators, try to parse number sequences,
/// and replace them with their digit form.
fn replace_numbers(text: &str, parser: fn(&[&str]) -> Option<(u64, usize)>) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }

    let mut result = String::new();
    let mut i = 0;
    let mut text_pos = 0;

    while i < words.len() {
        // Find word position in original text
        let word_pos = text[text_pos..].find(words[i]).map(|p| text_pos + p).unwrap_or(text_pos);
        // Append any text before this word (spaces, punctuation)
        result.push_str(&text[text_pos..word_pos]);

        // Try to parse a number starting at this position
        if let Some((value, consumed)) = parser(&words[i..]) {
            // Don't convert standalone "un"/"une"/"a"/"one" — too ambiguous
            if consumed == 1 && value <= 1 {
                result.push_str(words[i]);
                text_pos = word_pos + words[i].len();
                i += 1;
                continue;
            }

            result.push_str(&value.to_string());

            // Advance past consumed words
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

    // Append any trailing text
    if text_pos < text.len() {
        result.push_str(&text[text_pos..]);
    }

    result
}

// ---------------------------------------------------------------------------
// Ordinals
// ---------------------------------------------------------------------------

static RE_ORDINAL_FR: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\bpremi(?:er|ère)\b").unwrap(), "1er"),
        (Regex::new(r"(?i)\bdeuxi[eè]me\b").unwrap(), "2e"),
        (Regex::new(r"(?i)\bsecond(?:e)?\b").unwrap(), "2e"),
        (Regex::new(r"(?i)\btroisième\b").unwrap(), "3e"),
        (Regex::new(r"(?i)\bquatrième\b").unwrap(), "4e"),
        (Regex::new(r"(?i)\bcinquième\b").unwrap(), "5e"),
        (Regex::new(r"(?i)\bsixième\b").unwrap(), "6e"),
        (Regex::new(r"(?i)\bseptième\b").unwrap(), "7e"),
        (Regex::new(r"(?i)\bhuitième\b").unwrap(), "8e"),
        (Regex::new(r"(?i)\bneuvième\b").unwrap(), "9e"),
        (Regex::new(r"(?i)\bdixième\b").unwrap(), "10e"),
    ]
});

fn apply_ordinals_fr(text: &str) -> String {
    let mut result = text.to_string();
    for (re, replacement) in RE_ORDINAL_FR.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

static RE_ORDINAL_EN: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\bfirst\b").unwrap(), "1st"),
        (Regex::new(r"(?i)\bsecond\b").unwrap(), "2nd"),
        (Regex::new(r"(?i)\bthird\b").unwrap(), "3rd"),
        (Regex::new(r"(?i)\bfourth\b").unwrap(), "4th"),
        (Regex::new(r"(?i)\bfifth\b").unwrap(), "5th"),
        (Regex::new(r"(?i)\bsixth\b").unwrap(), "6th"),
        (Regex::new(r"(?i)\bseventh\b").unwrap(), "7th"),
        (Regex::new(r"(?i)\beighth\b").unwrap(), "8th"),
        (Regex::new(r"(?i)\bninth\b").unwrap(), "9th"),
        (Regex::new(r"(?i)\btenth\b").unwrap(), "10th"),
    ]
});

fn apply_ordinals_en(text: &str) -> String {
    let mut result = text.to_string();
    for (re, replacement) in RE_ORDINAL_EN.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

// ---------------------------------------------------------------------------
// Percentages — must run before cardinals to capture "X pour cent" / "X percent"
// ---------------------------------------------------------------------------

static RE_PCT_FR: LazyLock<Regex> = LazyLock::new(|| {
    // Matches number words + "pour cent" — we handle this by replacing "pour cent" after numbers
    Regex::new(r"(?i)\bpour cent\b").unwrap()
});

fn apply_percentages_fr(text: &str) -> String {
    // Replace "pour cent" with "%" — the preceding number will be handled by cardinals
    RE_PCT_FR.replace_all(text, "%").to_string()
}

static RE_PCT_EN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bpercent\b").unwrap()
});

fn apply_percentages_en(text: &str) -> String {
    RE_PCT_EN.replace_all(text, "%").to_string()
}

// ---------------------------------------------------------------------------
// Hours
// ---------------------------------------------------------------------------

static RE_HOURS_FR: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        // "X heure(s) et quart" → will be handled compositionally
        (Regex::new(r"(?i)\bet quart\b").unwrap(), "15"),
        (Regex::new(r"(?i)\bet demie?\b").unwrap(), "30"),
        (Regex::new(r"(?i)\bmoins le quart\b").unwrap(), "45"), // approximation
    ]
});

static RE_HEURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bheures?\b").unwrap()
});

fn apply_hours_fr(text: &str) -> String {
    let mut result = text.to_string();
    // First replace "et quart", "et demie" etc.
    for (re, replacement) in RE_HOURS_FR.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    // Replace "heure(s)" with "h"
    result = RE_HEURE.replace_all(&result, "h").to_string();
    result
}

static RE_HOURS_EN: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\bo'clock\b").unwrap(), ":00"),
        (Regex::new(r"(?i)\ba\.m\.\b").unwrap(), " AM"),
        (Regex::new(r"(?i)\bp\.m\.\b").unwrap(), " PM"),
        (Regex::new(r"(?i)(\d)\s*\bam\b").unwrap(), "$1 AM"),
        (Regex::new(r"(?i)(\d)\s*\bpm\b").unwrap(), "$1 PM"),
        (Regex::new(r"(?i)\ba quarter past\b").unwrap(), ":15"),
        (Regex::new(r"(?i)\bhalf past\b").unwrap(), ":30"),
        (Regex::new(r"(?i)\ba quarter to\b").unwrap(), ":45"),
    ]
});

fn apply_hours_en(text: &str) -> String {
    let mut result = text.to_string();
    for (re, replacement) in RE_HOURS_EN.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

// ---------------------------------------------------------------------------
// Currencies
// ---------------------------------------------------------------------------

static RE_CURRENCY_FR: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\beuros?\b").unwrap(), "\u{20AC}"),
        (Regex::new(r"(?i)\bdollars?\b").unwrap(), "$"),
        (Regex::new(r"(?i)\blivres? sterling\b").unwrap(), "\u{00A3}"),
        (Regex::new(r"(?i)\blivres?\b").unwrap(), "\u{00A3}"),
    ]
});

fn apply_currencies_fr(text: &str) -> String {
    let mut result = text.to_string();
    for (re, replacement) in RE_CURRENCY_FR.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

static RE_CURRENCY_EN: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\bdollars?\b").unwrap(), "$"),
        (Regex::new(r"(?i)\beuros?\b").unwrap(), "\u{20AC}"),
        (Regex::new(r"(?i)\bpounds?\b").unwrap(), "\u{00A3}"),
    ]
});

fn apply_currencies_en(text: &str) -> String {
    let mut result = text.to_string();
    for (re, replacement) in RE_CURRENCY_EN.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

// ---------------------------------------------------------------------------
// Units
// ---------------------------------------------------------------------------

static RE_UNITS_FR: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\bkilomètres?\b").unwrap(), "km"),
        (Regex::new(r"(?i)\bmètres?\b").unwrap(), "m"),
        (Regex::new(r"(?i)\bcentimètres?\b").unwrap(), "cm"),
        (Regex::new(r"(?i)\bmillimètres?\b").unwrap(), "mm"),
        (Regex::new(r"(?i)\bkilogrammes?\b").unwrap(), "kg"),
        (Regex::new(r"(?i)\bkilos?\b").unwrap(), "kg"),
        (Regex::new(r"(?i)\bgrammes?\b").unwrap(), "g"),
        (Regex::new(r"(?i)\blitres?\b").unwrap(), "L"),
        (Regex::new(r"(?i)\bmillilitres?\b").unwrap(), "mL"),
        (Regex::new(r"(?i)\bdegrés?\b").unwrap(), "\u{00B0}"),
    ]
});

fn apply_units_fr(text: &str) -> String {
    let mut result = text.to_string();
    for (re, replacement) in RE_UNITS_FR.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

static RE_UNITS_EN: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\bkilometers?\b").unwrap(), "km"),
        (Regex::new(r"(?i)\bmeters?\b").unwrap(), "m"),
        (Regex::new(r"(?i)\bcentimeters?\b").unwrap(), "cm"),
        (Regex::new(r"(?i)\bmillimeters?\b").unwrap(), "mm"),
        (Regex::new(r"(?i)\bkilograms?\b").unwrap(), "kg"),
        (Regex::new(r"(?i)\bgrams?\b").unwrap(), "g"),
        (Regex::new(r"(?i)\bliters?\b").unwrap(), "L"),
        (Regex::new(r"(?i)\bmilliliters?\b").unwrap(), "mL"),
        (Regex::new(r"(?i)\bmiles?\b").unwrap(), "mi"),
        (Regex::new(r"(?i)\bfeet\b").unwrap(), "ft"),
        (Regex::new(r"(?i)\binches?\b").unwrap(), "in"),
        (Regex::new(r"(?i)\bdegrees?\b").unwrap(), "\u{00B0}"),
    ]
});

fn apply_units_en(text: &str) -> String {
    let mut result = text.to_string();
    for (re, replacement) in RE_UNITS_EN.iter() {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- French cardinals --

    #[test]
    fn test_fr_simple_numbers() {
        assert_eq!(apply_itn("j'ai cinq chats", "fr"), "j'ai 5 chats");
        assert_eq!(apply_itn("il y a douze personnes", "fr"), "il y a 12 personnes");
        assert_eq!(apply_itn("seize ans", "fr"), "16 ans");
    }

    #[test]
    fn test_fr_compound_numbers() {
        assert_eq!(apply_itn("vingt-trois", "fr"), "23");
        assert_eq!(apply_itn("vingt et un", "fr"), "21");
        assert_eq!(apply_itn("soixante-dix", "fr"), "70");
        assert_eq!(apply_itn("quatre-vingt-dix-sept", "fr"), "97");
        assert_eq!(apply_itn("quatre-vingts", "fr"), "80");
    }

    #[test]
    fn test_fr_hundreds() {
        assert_eq!(apply_itn("cent", "fr"), "100");
        assert_eq!(apply_itn("deux cents", "fr"), "200");
        assert_eq!(apply_itn("trois cent vingt-et-un", "fr"), "321");
    }

    #[test]
    fn test_fr_thousands() {
        assert_eq!(apply_itn("mille", "fr"), "1000");
        assert_eq!(apply_itn("deux mille", "fr"), "2000");
        assert_eq!(apply_itn("trois mille deux cents", "fr"), "3200");
    }

    #[test]
    fn test_fr_standalone_un_not_converted() {
        // "un" alone is too ambiguous (article)
        assert_eq!(apply_itn("un chat", "fr"), "un chat");
    }

    // -- English cardinals --

    #[test]
    fn test_en_simple_numbers() {
        assert_eq!(apply_itn("I have five cats", "en"), "I have 5 cats");
        assert_eq!(apply_itn("there are twelve people", "en"), "there are 12 people");
    }

    #[test]
    fn test_en_compound_numbers() {
        assert_eq!(apply_itn("twenty-three", "en"), "23");
        assert_eq!(apply_itn("twenty three", "en"), "23");
        assert_eq!(apply_itn("ninety seven", "en"), "97");
    }

    #[test]
    fn test_en_hundreds() {
        assert_eq!(apply_itn("one hundred", "en"), "100");
        assert_eq!(apply_itn("two hundred and fifty", "en"), "250");
        assert_eq!(apply_itn("three hundred twenty one", "en"), "321");
    }

    #[test]
    fn test_en_thousands() {
        assert_eq!(apply_itn("one thousand", "en"), "1000");
        assert_eq!(apply_itn("two thousand", "en"), "2000");
        assert_eq!(apply_itn("three thousand two hundred", "en"), "3200");
    }

    // -- Ordinals --

    #[test]
    fn test_fr_ordinals() {
        assert_eq!(apply_itn("le premier janvier", "fr"), "le 1er janvier");
        assert_eq!(apply_itn("la deuxième fois", "fr"), "la 2e fois");
    }

    #[test]
    fn test_en_ordinals() {
        assert_eq!(apply_itn("the first of January", "en"), "the 1st of January");
        assert_eq!(apply_itn("the third time", "en"), "the 3rd time");
    }

    // -- Percentages --

    #[test]
    fn test_fr_percentages() {
        assert_eq!(apply_itn("dix pour cent", "fr"), "10 %");
    }

    #[test]
    fn test_en_percentages() {
        assert_eq!(apply_itn("ten percent", "en"), "10 %");
    }

    // -- Hours --

    #[test]
    fn test_fr_hours() {
        assert_eq!(apply_itn("trois heures", "fr"), "3 h");
        assert_eq!(apply_itn("trois heures et quart", "fr"), "3 h 15");
        assert_eq!(apply_itn("trois heures et demie", "fr"), "3 h 30");
    }

    #[test]
    fn test_en_hours() {
        assert_eq!(apply_itn("three o'clock", "en"), "3 :00");
    }

    // -- Currencies --

    #[test]
    fn test_fr_currencies() {
        assert_eq!(apply_itn("cinq euros", "fr"), "5 \u{20AC}");
    }

    #[test]
    fn test_en_currencies() {
        assert_eq!(apply_itn("five dollars", "en"), "5 $");
    }

    // -- Units --

    #[test]
    fn test_fr_units() {
        assert_eq!(apply_itn("deux kilomètres", "fr"), "2 km");
        assert_eq!(apply_itn("trois kilos", "fr"), "3 kg");
    }

    #[test]
    fn test_en_units() {
        assert_eq!(apply_itn("two kilometers", "en"), "2 km");
        assert_eq!(apply_itn("five miles", "en"), "5 mi");
    }

    // -- Mixed --

    #[test]
    fn test_fr_mixed() {
        assert_eq!(
            apply_itn("j'ai vingt-trois ans et je fais soixante-dix kilos", "fr"),
            "j'ai 23 ans et je fais 70 kg"
        );
    }

    #[test]
    fn test_en_mixed() {
        assert_eq!(
            apply_itn("I am twenty three years old and weigh one hundred fifty pounds", "en"),
            "I am 23 years old and weigh 150 \u{00A3}"
        );
    }

    #[test]
    fn test_no_change_on_regular_text() {
        assert_eq!(apply_itn("bonjour le monde", "fr"), "bonjour le monde");
        assert_eq!(apply_itn("hello world", "en"), "hello world");
    }

    // -- Edge cases --

    #[test]
    fn test_empty_input() {
        assert_eq!(apply_itn("", "fr"), "");
        assert_eq!(apply_itn("  ", "en"), "  ");
    }

    #[test]
    fn test_fr_million() {
        assert_eq!(apply_itn("deux millions", "fr"), "2000000");
    }

    #[test]
    fn test_en_million() {
        assert_eq!(apply_itn("three million", "en"), "3000000");
    }

    #[test]
    fn test_fr_complex_number() {
        assert_eq!(apply_itn("mille deux cent trente-quatre", "fr"), "1234");
    }

    #[test]
    fn test_en_complex_number() {
        assert_eq!(apply_itn("one thousand two hundred and thirty four", "en"), "1234");
    }

    #[test]
    fn test_multiple_numbers_in_sentence() {
        assert_eq!(apply_itn("j'ai cinq chats et trois chiens", "fr"), "j'ai 5 chats et 3 chiens");
    }

    #[test]
    fn test_fr_degrees() {
        assert_eq!(apply_itn("vingt degrés", "fr"), "20 \u{00B0}");
    }

    #[test]
    fn test_en_degrees() {
        assert_eq!(apply_itn("seventy degrees", "en"), "70 \u{00B0}");
    }

    #[test]
    fn test_lang_prefix_routes() {
        // "fr-FR" should use French rules
        assert_eq!(apply_itn("cinq euros", "fr-FR"), "5 \u{20AC}");
        // "en-US" should use English rules
        assert_eq!(apply_itn("five dollars", "en-US"), "5 $");
    }
}
