//! Inverse Text Normalization (ITN) — converts spoken forms to written canonical forms.
//!
//! Supports 9 languages: FR, EN, DE, ES, PT, IT, NL, PL, RU.
//! Each language has: cardinal numbers, ordinals, percentages, hours, currencies, units.

use regex::Regex;
use std::sync::LazyLock;

/// Extract base language code: "fr-CA" → "fr"
fn lang_base(language: &str) -> &str {
    language.split(&['-', '_'][..]).next().unwrap_or(language)
}

/// Apply ITN transformations to text.
pub fn apply_itn(text: &str, language: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    let lang = lang_base(language);
    let mut result = text.to_string();

    match lang {
        "fr" => {
            result = apply_percentages_fr(&result);
            result = apply_hours_fr(&result);
            result = apply_currencies_fr(&result);
            result = apply_ordinals_fr(&result);
            result = apply_units_fr(&result);
            result = apply_cardinals_fr(&result);
        }
        "de" => {
            result = apply_regex_list(&result, &RE_PCT_DE);
            result = apply_regex_list(&result, &RE_HOURS_DE);
            result = apply_regex_list(&result, &RE_CURRENCY_DE);
            result = apply_regex_list(&result, &RE_ORDINAL_DE);
            result = apply_regex_list(&result, &RE_UNITS_DE);
            result = apply_cardinals_de(&result);
        }
        "es" => {
            result = apply_regex_list(&result, &RE_PCT_ES);
            result = apply_regex_list(&result, &RE_HOURS_ES);
            result = apply_regex_list(&result, &RE_CURRENCY_ES);
            result = apply_regex_list(&result, &RE_ORDINAL_ES);
            result = apply_regex_list(&result, &RE_UNITS_ES);
            result = apply_cardinals_es(&result);
        }
        "pt" => {
            result = apply_regex_list(&result, &RE_PCT_PT);
            result = apply_regex_list(&result, &RE_HOURS_PT);
            result = apply_regex_list(&result, &RE_CURRENCY_PT);
            result = apply_regex_list(&result, &RE_ORDINAL_PT);
            result = apply_regex_list(&result, &RE_UNITS_PT);
            result = apply_cardinals_pt(&result);
        }
        "it" => {
            result = apply_regex_list(&result, &RE_PCT_IT);
            result = apply_regex_list(&result, &RE_HOURS_IT);
            result = apply_regex_list(&result, &RE_CURRENCY_IT);
            result = apply_regex_list(&result, &RE_ORDINAL_IT);
            result = apply_regex_list(&result, &RE_UNITS_IT);
            result = apply_cardinals_it(&result);
        }
        "nl" => {
            result = apply_regex_list(&result, &RE_PCT_NL);
            result = apply_regex_list(&result, &RE_HOURS_NL);
            result = apply_regex_list(&result, &RE_CURRENCY_NL);
            result = apply_regex_list(&result, &RE_ORDINAL_NL);
            result = apply_regex_list(&result, &RE_UNITS_NL);
            result = apply_cardinals_nl(&result);
        }
        "pl" => {
            result = apply_regex_list(&result, &RE_PCT_PL);
            result = apply_regex_list(&result, &RE_HOURS_PL);
            result = apply_regex_list(&result, &RE_CURRENCY_PL);
            result = apply_regex_list(&result, &RE_ORDINAL_PL);
            result = apply_regex_list(&result, &RE_UNITS_PL);
            result = apply_cardinals_pl(&result);
        }
        "ru" => {
            result = apply_regex_list(&result, &RE_PCT_RU);
            result = apply_regex_list(&result, &RE_HOURS_RU);
            result = apply_regex_list(&result, &RE_CURRENCY_RU);
            result = apply_regex_list(&result, &RE_ORDINAL_RU);
            result = apply_regex_list(&result, &RE_UNITS_RU);
            result = apply_cardinals_ru(&result);
        }
        _ => {
            result = apply_percentages_en(&result);
            result = apply_hours_en(&result);
            result = apply_currencies_en(&result);
            result = apply_ordinals_en(&result);
            result = apply_units_en(&result);
            result = apply_cardinals_en(&result);
        }
    }

    result
}

/// Apply a list of (regex, replacement) pairs to text.
fn apply_regex_list(text: &str, rules: &[(Regex, &str)]) -> String {
    let mut result = text.to_string();
    for (re, replacement) in rules {
        result = re.replace_all(&result, *replacement).to_string();
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
// German (DE)
// ---------------------------------------------------------------------------

fn de_atom(word: &str) -> Option<u64> {
    match word {
        "null" => Some(0),
        "eins" | "ein" | "eine" => Some(1),
        "zwei" | "zwo" => Some(2),
        "drei" => Some(3),
        "vier" => Some(4),
        "fünf" | "fuenf" => Some(5),
        "sechs" => Some(6),
        "sieben" => Some(7),
        "acht" => Some(8),
        "neun" => Some(9),
        "zehn" => Some(10),
        "elf" => Some(11),
        "zwölf" | "zwoelf" => Some(12),
        "dreizehn" => Some(13),
        "vierzehn" => Some(14),
        "fünfzehn" | "fuenfzehn" => Some(15),
        "sechzehn" => Some(16),
        "siebzehn" => Some(17),
        "achtzehn" => Some(18),
        "neunzehn" => Some(19),
        "zwanzig" => Some(20),
        "dreißig" | "dreissig" => Some(30),
        "vierzig" => Some(40),
        "fünfzig" | "fuenfzig" => Some(50),
        "sechzig" => Some(60),
        "siebzig" => Some(70),
        "achtzig" => Some(80),
        "neunzig" => Some(90),
        _ => None,
    }
}

fn de_multiplier(word: &str) -> Option<u64> {
    match word {
        "hundert" => Some(100),
        "tausend" => Some(1_000),
        "million" | "millionen" => Some(1_000_000),
        "milliarde" | "milliarden" => Some(1_000_000_000),
        _ => None,
    }
}

/// Try to decompose a German compound number word: "dreiundzwanzig" → 23, "zweihundert" → 200
fn de_decompose_compound(word: &str) -> Option<u64> {
    // Split on "und" — unit+und+tens: "dreiundzwanzig" → 23
    if let Some(pos) = word.find("und") {
        let left = &word[..pos];
        let right = &word[pos + 3..];
        if let (Some(unit), Some(tens)) = (de_atom(left), de_atom(right)) {
            if unit < 10 && (20..=90).contains(&tens) && tens % 10 == 0 {
                return Some(tens + unit);
            }
        }
    }
    // "zweihundert" → 200, "dreihundert" → 300
    if let Some(prefix) = word.strip_suffix("hundert") {
        if !prefix.is_empty() {
            return de_atom(prefix).map(|n| n * 100);
        }
    }
    // "zweitausend" → 2000
    if let Some(prefix) = word.strip_suffix("tausend") {
        if !prefix.is_empty() {
            return de_atom(prefix).map(|n| n * 1000);
        }
    }
    None
}

fn parse_de_number(words: &[&str]) -> Option<(u64, usize)> {
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

        // Try compound word first (e.g. "dreiundzwanzig")
        if let Some(val) = de_decompose_compound(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }

        if let Some(val) = de_atom(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }

        if let Some(mult) = de_multiplier(w) {
            consumed_any = true;
            if mult >= 1_000_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * mult;
                current_group = 0;
            } else if mult == 1_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * 1_000;
                current_group = 0;
            } else {
                let coef = if current_group == 0 { 1 } else { current_group };
                current_group = coef * 100;
            }
            pos += 1;
            continue;
        }

        break;
    }

    if !consumed_any { return None; }
    total += current_group;
    Some((total, pos))
}

fn apply_cardinals_de(text: &str) -> String { replace_numbers(text, parse_de_number) }

macro_rules! regex_rules {
    ($name:ident, [ $(($pat:expr, $rep:expr)),* $(,)? ]) => {
        static $name: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
            vec![ $( (Regex::new($pat).unwrap(), $rep), )* ]
        });
    };
}

regex_rules!(RE_PCT_DE, [
    (r"(?i)\bProzent\b", "%")
]);
regex_rules!(RE_HOURS_DE, [
    (r"(?i)\bUhr\b", "h"),
    (r"(?i)\bViertel nach\b", ":15"),
    (r"(?i)\bViertel vor\b", ":45")
]);
regex_rules!(RE_CURRENCY_DE, [
    (r"(?i)\bEuros?\b", "\u{20AC}"),
    (r"(?i)\bDollars?\b", "$"),
    (r"(?i)\bFranken\b", "CHF"),
    (r"(?i)\bPfund\b", "\u{00A3}")
]);
regex_rules!(RE_ORDINAL_DE, [
    (r"(?i)\berste[rnms]?\b", "1."),
    (r"(?i)\bzweite[rnms]?\b", "2."),
    (r"(?i)\bdritte[rnms]?\b", "3."),
    (r"(?i)\bvierte[rnms]?\b", "4."),
    (r"(?i)\bfünfte[rnms]?\b", "5."),
    (r"(?i)\bsechste[rnms]?\b", "6."),
    (r"(?i)\bsiebte[rnms]?\b", "7."),
    (r"(?i)\bachte[rnms]?\b", "8."),
    (r"(?i)\bneunte[rnms]?\b", "9."),
    (r"(?i)\bzehnte[rnms]?\b", "10.")
]);
regex_rules!(RE_UNITS_DE, [
    (r"(?i)\bKilometer\b", "km"),
    (r"(?i)\bMeter\b", "m"),
    (r"(?i)\bZentimeter\b", "cm"),
    (r"(?i)\bMillimeter\b", "mm"),
    (r"(?i)\bKilogramm\b", "kg"),
    (r"(?i)\bKilos?\b", "kg"),
    (r"(?i)\bGramm\b", "g"),
    (r"(?i)\bLiters?\b", "L"),
    (r"(?i)\bMilliliters?\b", "mL"),
    (r"(?i)\bGrad\b", "\u{00B0}")
]);

// ---------------------------------------------------------------------------
// Spanish (ES)
// ---------------------------------------------------------------------------

fn es_atom(word: &str) -> Option<u64> {
    match word {
        "cero" => Some(0),
        "uno" | "una" | "un" => Some(1),
        "dos" => Some(2),
        "tres" => Some(3),
        "cuatro" => Some(4),
        "cinco" => Some(5),
        "seis" => Some(6),
        "siete" => Some(7),
        "ocho" => Some(8),
        "nueve" => Some(9),
        "diez" => Some(10),
        "once" => Some(11),
        "doce" => Some(12),
        "trece" => Some(13),
        "catorce" => Some(14),
        "quince" => Some(15),
        "dieciséis" | "dieciseis" => Some(16),
        "diecisiete" => Some(17),
        "dieciocho" => Some(18),
        "diecinueve" => Some(19),
        "veinte" => Some(20),
        "veintiuno" | "veintiuna" | "veintiún" => Some(21),
        "veintidós" | "veintidos" => Some(22),
        "veintitrés" | "veintitres" => Some(23),
        "veinticuatro" => Some(24),
        "veinticinco" => Some(25),
        "veintiséis" | "veintiseis" => Some(26),
        "veintisiete" => Some(27),
        "veintiocho" => Some(28),
        "veintinueve" => Some(29),
        "treinta" => Some(30),
        "cuarenta" => Some(40),
        "cincuenta" => Some(50),
        "sesenta" => Some(60),
        "setenta" => Some(70),
        "ochenta" => Some(80),
        "noventa" => Some(90),
        _ => None,
    }
}

fn es_multiplier(word: &str) -> Option<u64> {
    match word {
        "cien" | "ciento" => Some(100),
        "doscientos" | "doscientas" => Some(200),
        "trescientos" | "trescientas" => Some(300),
        "cuatrocientos" | "cuatrocientas" => Some(400),
        "quinientos" | "quinientas" => Some(500),
        "seiscientos" | "seiscientas" => Some(600),
        "setecientos" | "setecientas" => Some(700),
        "ochocientos" | "ochocientas" => Some(800),
        "novecientos" | "novecientas" => Some(900),
        "mil" => Some(1_000),
        "millón" | "millon" | "millones" => Some(1_000_000),
        _ => None,
    }
}

fn parse_es_number(words: &[&str]) -> Option<(u64, usize)> {
    if words.is_empty() { return None; }
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let mut pos = 0;
    let mut total: u64 = 0;
    let mut current_group: u64 = 0;
    let mut consumed_any = false;

    while pos < lower.len() {
        let w = lower[pos].as_str();

        if w == "y" {
            if consumed_any && pos + 1 < lower.len() { pos += 1; continue; }
            break;
        }

        if let Some(val) = es_atom(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }

        if let Some(mult) = es_multiplier(w) {
            consumed_any = true;
            if (200..=900).contains(&mult) {
                total += mult;
                pos += 1;
                continue;
            }
            if mult >= 1_000_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * mult;
                current_group = 0;
            } else if mult == 1_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * 1_000;
                current_group = 0;
            } else {
                let coef = if current_group == 0 { 1 } else { current_group };
                current_group = coef * 100;
            }
            pos += 1;
            continue;
        }
        break;
    }

    if !consumed_any { return None; }
    total += current_group;
    Some((total, pos))
}

fn apply_cardinals_es(text: &str) -> String { replace_numbers(text, parse_es_number) }

regex_rules!(RE_PCT_ES, [(r"(?i)\bpor ciento\b", "%")]);
regex_rules!(RE_HOURS_ES, [
    (r"(?i)\ben punto\b", ":00"),
    (r"(?i)\by media\b", ":30"),
    (r"(?i)\by cuarto\b", ":15"),
    (r"(?i)\bmenos cuarto\b", ":45")
]);
regex_rules!(RE_CURRENCY_ES, [
    (r"(?i)\beuros?\b", "\u{20AC}"),
    (r"(?i)\bdólares?\b", "$"),
    (r"(?i)\bdolares?\b", "$"),
    (r"(?i)\blibras?\b", "\u{00A3}"),
    (r"(?i)\bpesos?\b", "$")
]);
regex_rules!(RE_ORDINAL_ES, [
    (r"(?i)\bprimero\b", "1.º"),
    (r"(?i)\bprimera\b", "1.ª"),
    (r"(?i)\bsegundo\b", "2.º"),
    (r"(?i)\bsegunda\b", "2.ª"),
    (r"(?i)\btercero\b", "3.º"),
    (r"(?i)\btercera\b", "3.ª"),
    (r"(?i)\bcuarto\b", "4.º"),
    (r"(?i)\bquinto\b", "5.º"),
    (r"(?i)\bsexto\b", "6.º"),
    (r"(?i)\bséptimo\b", "7.º"),
    (r"(?i)\boctavo\b", "8.º"),
    (r"(?i)\bnoveno\b", "9.º"),
    (r"(?i)\bdécimo\b", "10.º")
]);
regex_rules!(RE_UNITS_ES, [
    (r"(?i)\bkilómetros?\b", "km"),
    (r"(?i)\bkilometros?\b", "km"),
    (r"(?i)\bmetros?\b", "m"),
    (r"(?i)\bcentímetros?\b", "cm"),
    (r"(?i)\bcentimetros?\b", "cm"),
    (r"(?i)\bmilímetros?\b", "mm"),
    (r"(?i)\bmilimetros?\b", "mm"),
    (r"(?i)\bkilogramos?\b", "kg"),
    (r"(?i)\bkilos?\b", "kg"),
    (r"(?i)\bgramos?\b", "g"),
    (r"(?i)\blitros?\b", "L"),
    (r"(?i)\bgrados?\b", "\u{00B0}")
]);

// ---------------------------------------------------------------------------
// Portuguese (PT)
// ---------------------------------------------------------------------------

fn pt_atom(word: &str) -> Option<u64> {
    match word {
        "zero" => Some(0),
        "um" | "uma" => Some(1),
        "dois" | "duas" => Some(2),
        "três" | "tres" => Some(3),
        "quatro" => Some(4),
        "cinco" => Some(5),
        "seis" => Some(6),
        "sete" => Some(7),
        "oito" => Some(8),
        "nove" => Some(9),
        "dez" => Some(10),
        "onze" => Some(11),
        "doze" => Some(12),
        "treze" => Some(13),
        "catorze" | "quatorze" => Some(14),
        "quinze" => Some(15),
        "dezesseis" | "dezasseis" => Some(16),
        "dezessete" | "dezassete" => Some(17),
        "dezoito" => Some(18),
        "dezenove" | "dezanove" => Some(19),
        "vinte" => Some(20),
        "trinta" => Some(30),
        "quarenta" => Some(40),
        "cinquenta" => Some(50),
        "sessenta" => Some(60),
        "setenta" => Some(70),
        "oitenta" => Some(80),
        "noventa" => Some(90),
        _ => None,
    }
}

fn pt_multiplier(word: &str) -> Option<u64> {
    match word {
        "cem" | "cento" => Some(100),
        "duzentos" | "duzentas" => Some(200),
        "trezentos" | "trezentas" => Some(300),
        "quatrocentos" | "quatrocentas" => Some(400),
        "quinhentos" | "quinhentas" => Some(500),
        "seiscentos" | "seiscentas" => Some(600),
        "setecentos" | "setecentas" => Some(700),
        "oitocentos" | "oitocentas" => Some(800),
        "novecentos" | "novecentas" => Some(900),
        "mil" => Some(1_000),
        "milhão" | "milhao" | "milhões" | "milhoes" => Some(1_000_000),
        _ => None,
    }
}

fn parse_pt_number(words: &[&str]) -> Option<(u64, usize)> {
    if words.is_empty() { return None; }
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let mut pos = 0;
    let mut total: u64 = 0;
    let mut current_group: u64 = 0;
    let mut consumed_any = false;

    while pos < lower.len() {
        let w = lower[pos].as_str();
        if w == "e" {
            if consumed_any && pos + 1 < lower.len() { pos += 1; continue; }
            break;
        }
        if let Some(val) = pt_atom(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }
        if let Some(mult) = pt_multiplier(w) {
            consumed_any = true;
            if (200..=900).contains(&mult) {
                total += mult;
                pos += 1;
                continue;
            }
            if mult >= 1_000_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * mult;
                current_group = 0;
            } else if mult == 1_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * 1_000;
                current_group = 0;
            } else {
                let coef = if current_group == 0 { 1 } else { current_group };
                current_group = coef * 100;
            }
            pos += 1;
            continue;
        }
        break;
    }

    if !consumed_any { return None; }
    total += current_group;
    Some((total, pos))
}

fn apply_cardinals_pt(text: &str) -> String { replace_numbers(text, parse_pt_number) }

regex_rules!(RE_PCT_PT, [(r"(?i)\bpor cento\b", "%")]);
regex_rules!(RE_HOURS_PT, [
    (r"(?i)\bem ponto\b", ":00"),
    (r"(?i)\be meia\b", ":30"),
    (r"(?i)\bhoras?\b", "h")
]);
regex_rules!(RE_CURRENCY_PT, [
    (r"(?i)\beuros?\b", "\u{20AC}"),
    (r"(?i)\bdólares?\b", "$"),
    (r"(?i)\bdolares?\b", "$"),
    (r"(?i)\blibras?\b", "\u{00A3}"),
    (r"(?i)\breais\b", "R$"),
    (r"(?i)\breal\b", "R$")
]);
regex_rules!(RE_ORDINAL_PT, [
    (r"(?i)\bprimeiro\b", "1.º"),
    (r"(?i)\bprimeira\b", "1.ª"),
    (r"(?i)\bsegundo\b", "2.º"),
    (r"(?i)\bsegunda\b", "2.ª"),
    (r"(?i)\bterceiro\b", "3.º"),
    (r"(?i)\bterceira\b", "3.ª"),
    (r"(?i)\bquarto\b", "4.º"),
    (r"(?i)\bquinto\b", "5.º"),
    (r"(?i)\bsexto\b", "6.º"),
    (r"(?i)\bsétimo\b", "7.º"),
    (r"(?i)\boitavo\b", "8.º"),
    (r"(?i)\bnono\b", "9.º"),
    (r"(?i)\bdécimo\b", "10.º")
]);
regex_rules!(RE_UNITS_PT, [
    (r"(?i)\bquilómetros?\b", "km"),
    (r"(?i)\bquilometros?\b", "km"),
    (r"(?i)\bmetros?\b", "m"),
    (r"(?i)\bcentímetros?\b", "cm"),
    (r"(?i)\bcentimetros?\b", "cm"),
    (r"(?i)\bquilogramas?\b", "kg"),
    (r"(?i)\bquilos?\b", "kg"),
    (r"(?i)\bgramas?\b", "g"),
    (r"(?i)\blitros?\b", "L"),
    (r"(?i)\bgraus?\b", "\u{00B0}")
]);

// ---------------------------------------------------------------------------
// Italian (IT)
// ---------------------------------------------------------------------------

fn it_atom(word: &str) -> Option<u64> {
    match word {
        "zero" => Some(0),
        "uno" | "una" | "un" => Some(1),
        "due" => Some(2),
        "tre" | "tré" => Some(3),
        "quattro" => Some(4),
        "cinque" => Some(5),
        "sei" => Some(6),
        "sette" => Some(7),
        "otto" => Some(8),
        "nove" => Some(9),
        "dieci" => Some(10),
        "undici" => Some(11),
        "dodici" => Some(12),
        "tredici" => Some(13),
        "quattordici" => Some(14),
        "quindici" => Some(15),
        "sedici" => Some(16),
        "diciassette" => Some(17),
        "diciotto" => Some(18),
        "diciannove" => Some(19),
        "venti" => Some(20),
        "trenta" => Some(30),
        "quaranta" => Some(40),
        "cinquanta" => Some(50),
        "sessanta" => Some(60),
        "settanta" => Some(70),
        "ottanta" => Some(80),
        "novanta" => Some(90),
        _ => None,
    }
}

fn it_multiplier(word: &str) -> Option<u64> {
    match word {
        "cento" => Some(100),
        "mille" | "mila" => Some(1_000),
        "milione" | "milioni" => Some(1_000_000),
        "miliardo" | "miliardi" => Some(1_000_000_000),
        _ => None,
    }
}

/// Decompose Italian compound numbers: "ventitré" → 23, "trentuno" → 31, "duecento" → 200
fn it_decompose_compound(word: &str) -> Option<u64> {
    // Compound tens: ventitré, trentuno, novantasette
    let tens = [
        ("venti", 20), ("trenta", 30), ("quaranta", 40), ("cinquanta", 50),
        ("sessanta", 60), ("settanta", 70), ("ottanta", 80), ("novanta", 90),
    ];
    for &(prefix, tens_val) in &tens {
        // Full prefix: ventidue, trentasei, novantasette
        if let Some(suffix) = word.strip_prefix(prefix) {
            if let Some(unit) = it_atom(suffix) {
                if (1..=9).contains(&unit) {
                    return Some(tens_val + unit);
                }
            }
        }
        // Elided prefix (drop last vowel): ventuno, trentotto
        let elided = &prefix[..prefix.len() - 1];
        if let Some(suffix) = word.strip_prefix(elided) {
            if !suffix.is_empty() && !suffix.starts_with(&prefix[prefix.len() - 1..prefix.len()]) {
                if let Some(unit) = it_atom(suffix) {
                    if (1..=9).contains(&unit) {
                        return Some(tens_val + unit);
                    }
                }
            }
        }
    }
    // Compound hundreds: duecento, trecento, cinquecento
    if let Some(prefix) = word.strip_suffix("cento") {
        if !prefix.is_empty() {
            return it_atom(prefix).map(|n| n * 100);
        }
    }
    // Compound thousands: duemila, tremila
    if let Some(prefix) = word.strip_suffix("mila") {
        return it_atom(prefix).map(|n| n * 1000);
    }
    None
}

fn parse_it_number(words: &[&str]) -> Option<(u64, usize)> {
    if words.is_empty() { return None; }
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let mut pos = 0;
    let mut total: u64 = 0;
    let mut current_group: u64 = 0;
    let mut consumed_any = false;

    while pos < lower.len() {
        let w = lower[pos].as_str();
        if let Some(val) = it_decompose_compound(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }
        if let Some(val) = it_atom(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }
        if let Some(mult) = it_multiplier(w) {
            consumed_any = true;
            if mult >= 1_000_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * mult;
                current_group = 0;
            } else if mult == 1_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * 1_000;
                current_group = 0;
            } else {
                let coef = if current_group == 0 { 1 } else { current_group };
                current_group = coef * 100;
            }
            pos += 1;
            continue;
        }
        break;
    }

    if !consumed_any { return None; }
    total += current_group;
    Some((total, pos))
}

fn apply_cardinals_it(text: &str) -> String { replace_numbers(text, parse_it_number) }

regex_rules!(RE_PCT_IT, [(r"(?i)\bper cento\b", "%"), (r"(?i)\bpercento\b", "%")]);
regex_rules!(RE_HOURS_IT, [
    (r"(?i)\bin punto\b", ":00"),
    (r"(?i)\be mezza\b", ":30"),
    (r"(?i)\be mezzo\b", ":30"),
    (r"(?i)\bore\b", "h"),
    (r"(?i)\bora\b", "h")
]);
regex_rules!(RE_CURRENCY_IT, [
    (r"(?i)\beuro\b", "\u{20AC}"),
    (r"(?i)\bdollaro\b", "$"),
    (r"(?i)\bdollari\b", "$"),
    (r"(?i)\bsterlina\b", "\u{00A3}"),
    (r"(?i)\bsterline\b", "\u{00A3}")
]);
regex_rules!(RE_ORDINAL_IT, [
    (r"(?i)\bprimo\b", "1.º"),
    (r"(?i)\bprima\b", "1.ª"),
    (r"(?i)\bsecondo\b", "2.º"),
    (r"(?i)\bseconda\b", "2.ª"),
    (r"(?i)\bterzo\b", "3.º"),
    (r"(?i)\bterza\b", "3.ª"),
    (r"(?i)\bquarto\b", "4.º"),
    (r"(?i)\bquinto\b", "5.º"),
    (r"(?i)\bsesto\b", "6.º"),
    (r"(?i)\bsettimo\b", "7.º"),
    (r"(?i)\bottavo\b", "8.º"),
    (r"(?i)\bnono\b", "9.º"),
    (r"(?i)\bdecimo\b", "10.º")
]);
regex_rules!(RE_UNITS_IT, [
    (r"(?i)\bchilometri?\b", "km"),
    (r"(?i)\bmetri?\b", "m"),
    (r"(?i)\bcentimetri?\b", "cm"),
    (r"(?i)\bmillimetri?\b", "mm"),
    (r"(?i)\bchilogrammi?\b", "kg"),
    (r"(?i)\bchili?\b", "kg"),
    (r"(?i)\bgrammi?\b", "g"),
    (r"(?i)\blitri?\b", "L"),
    (r"(?i)\bgradi?\b", "\u{00B0}")
]);

// ---------------------------------------------------------------------------
// Dutch (NL)
// ---------------------------------------------------------------------------

fn nl_atom(word: &str) -> Option<u64> {
    match word {
        "nul" => Some(0),
        "een" | "één" => Some(1),
        "twee" => Some(2),
        "drie" => Some(3),
        "vier" => Some(4),
        "vijf" => Some(5),
        "zes" => Some(6),
        "zeven" => Some(7),
        "acht" => Some(8),
        "negen" => Some(9),
        "tien" => Some(10),
        "elf" => Some(11),
        "twaalf" => Some(12),
        "dertien" => Some(13),
        "veertien" => Some(14),
        "vijftien" => Some(15),
        "zestien" => Some(16),
        "zeventien" => Some(17),
        "achttien" => Some(18),
        "negentien" => Some(19),
        "twintig" => Some(20),
        "dertig" => Some(30),
        "veertig" => Some(40),
        "vijftig" => Some(50),
        "zestig" => Some(60),
        "zeventig" => Some(70),
        "tachtig" => Some(80),
        "negentig" => Some(90),
        _ => None,
    }
}

fn nl_multiplier(word: &str) -> Option<u64> {
    match word {
        "honderd" => Some(100),
        "duizend" => Some(1_000),
        "miljoen" | "miljoenen" => Some(1_000_000),
        "miljard" | "miljarden" => Some(1_000_000_000),
        _ => None,
    }
}

/// Decompose Dutch compound numbers: "drieëntwintig" → 23, "tweehonderd" → 200
fn nl_decompose_compound(word: &str) -> Option<u64> {
    // Dutch uses "ën" or "en" as connector: drieëntwintig, eenendertig
    // Must try all positions since "een" itself contains "en"
    for sep in &["\u{00EB}n", "en"] {
        let mut search_from = 0;
        while let Some(pos) = word[search_from..].find(sep) {
            let pos = search_from + pos;
            let left = &word[..pos];
            let right = &word[pos + sep.len()..];
            if let (Some(unit), Some(tens)) = (nl_atom(left), nl_atom(right)) {
                if unit < 10 && (20..=90).contains(&tens) && tens % 10 == 0 {
                    return Some(tens + unit);
                }
            }
            search_from = pos + 1;
        }
    }
    // Compound hundreds: tweehonderd → 200
    if let Some(prefix) = word.strip_suffix("honderd") {
        if !prefix.is_empty() {
            return nl_atom(prefix).map(|n| n * 100);
        }
    }
    // Compound thousands: tweeduizend → 2000
    if let Some(prefix) = word.strip_suffix("duizend") {
        if !prefix.is_empty() {
            return nl_atom(prefix).map(|n| n * 1000);
        }
    }
    None
}

fn parse_nl_number(words: &[&str]) -> Option<(u64, usize)> {
    if words.is_empty() { return None; }
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let mut pos = 0;
    let mut total: u64 = 0;
    let mut current_group: u64 = 0;
    let mut consumed_any = false;

    while pos < lower.len() {
        let w = lower[pos].as_str();
        if let Some(val) = nl_decompose_compound(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }
        if let Some(val) = nl_atom(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }
        if let Some(mult) = nl_multiplier(w) {
            consumed_any = true;
            if mult >= 1_000_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * mult;
                current_group = 0;
            } else if mult == 1_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * 1_000;
                current_group = 0;
            } else {
                let coef = if current_group == 0 { 1 } else { current_group };
                current_group = coef * 100;
            }
            pos += 1;
            continue;
        }
        break;
    }

    if !consumed_any { return None; }
    total += current_group;
    Some((total, pos))
}

fn apply_cardinals_nl(text: &str) -> String { replace_numbers(text, parse_nl_number) }

regex_rules!(RE_PCT_NL, [(r"(?i)\bprocent\b", "%")]);
regex_rules!(RE_HOURS_NL, [
    (r"(?i)\buur\b", "h"),
    (r"(?i)\bkwart over\b", ":15"),
    (r"(?i)\bkwart voor\b", ":45")
]);
regex_rules!(RE_CURRENCY_NL, [
    (r"(?i)\beuro(?:'s)?\b", "\u{20AC}"),
    (r"(?i)\bdollars?\b", "$"),
    (r"(?i)\bpond\b", "\u{00A3}")
]);
regex_rules!(RE_ORDINAL_NL, [
    (r"(?i)\beerste\b", "1e"),
    (r"(?i)\btweede\b", "2e"),
    (r"(?i)\bderde\b", "3e"),
    (r"(?i)\bvierde\b", "4e"),
    (r"(?i)\bvijfde\b", "5e"),
    (r"(?i)\bzesde\b", "6e"),
    (r"(?i)\bzevende\b", "7e"),
    (r"(?i)\bachtste\b", "8e"),
    (r"(?i)\bnegende\b", "9e"),
    (r"(?i)\btiende\b", "10e")
]);
regex_rules!(RE_UNITS_NL, [
    (r"(?i)\bkilometers?\b", "km"),
    (r"(?i)\bmeters?\b", "m"),
    (r"(?i)\bcentimeters?\b", "cm"),
    (r"(?i)\bmillimeters?\b", "mm"),
    (r"(?i)\bkilogrammen?\b", "kg"),
    (r"(?i)\bkilo(?:'s)?\b", "kg"),
    (r"(?i)\bgrammen?\b", "g"),
    (r"(?i)\bliters?\b", "L"),
    (r"(?i)\bgraden\b", "\u{00B0}"),
    (r"(?i)\bgraad\b", "\u{00B0}")
]);

// ---------------------------------------------------------------------------
// Polish (PL)
// ---------------------------------------------------------------------------

fn pl_atom(word: &str) -> Option<u64> {
    match word {
        "zero" => Some(0),
        "jeden" | "jedna" | "jedno" => Some(1),
        "dwa" | "dwie" => Some(2),
        "trzy" => Some(3),
        "cztery" => Some(4),
        "pięć" | "piec" => Some(5),
        "sześć" | "szesc" => Some(6),
        "siedem" => Some(7),
        "osiem" => Some(8),
        "dziewięć" | "dziewiec" => Some(9),
        "dziesięć" | "dziesiec" => Some(10),
        "jedenaście" | "jedenascie" => Some(11),
        "dwanaście" | "dwanascie" => Some(12),
        "trzynaście" | "trzynascie" => Some(13),
        "czternaście" | "czternascie" => Some(14),
        "piętnaście" | "pietnascie" => Some(15),
        "szesnaście" | "szesnascie" => Some(16),
        "siedemnaście" | "siedemnascie" => Some(17),
        "osiemnaście" | "osiemnascie" => Some(18),
        "dziewiętnaście" | "dziewietnascie" => Some(19),
        "dwadzieścia" | "dwadziescia" => Some(20),
        "trzydzieści" | "trzydziesci" => Some(30),
        "czterdzieści" | "czterdziesci" => Some(40),
        "pięćdziesiąt" | "piecdziesiat" => Some(50),
        "sześćdziesiąt" | "szescdziesiat" => Some(60),
        "siedemdziesiąt" | "siedemdziesiat" => Some(70),
        "osiemdziesiąt" | "osiemdziesiat" => Some(80),
        "dziewięćdziesiąt" | "dziewiecdziesiat" => Some(90),
        _ => None,
    }
}

fn pl_multiplier(word: &str) -> Option<u64> {
    match word {
        "sto" => Some(100),
        "dwieście" | "dwiescie" => Some(200),
        "trzysta" => Some(300),
        "czterysta" => Some(400),
        "pięćset" | "piecset" => Some(500),
        "sześćset" | "szescset" => Some(600),
        "siedemset" => Some(700),
        "osiemset" => Some(800),
        "dziewięćset" | "dziewiecset" => Some(900),
        "tysiąc" | "tysiac" | "tysiące" | "tysiace" | "tysięcy" | "tysiecy" => Some(1_000),
        "milion" | "miliony" | "milionów" | "milionow" => Some(1_000_000),
        "miliard" | "miliardy" | "miliardów" | "miliardow" => Some(1_000_000_000),
        _ => None,
    }
}

fn parse_pl_number(words: &[&str]) -> Option<(u64, usize)> {
    if words.is_empty() { return None; }
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let mut pos = 0;
    let mut total: u64 = 0;
    let mut current_group: u64 = 0;
    let mut consumed_any = false;

    while pos < lower.len() {
        let w = lower[pos].as_str();
        if let Some(val) = pl_atom(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }
        if let Some(mult) = pl_multiplier(w) {
            consumed_any = true;
            // Polish hundreds (200-900) are single words with specific values
            if (200..=900).contains(&mult) {
                total += mult;
                pos += 1;
                continue;
            }
            if mult >= 1_000_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * mult;
                current_group = 0;
            } else if mult == 1_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * 1_000;
                current_group = 0;
            } else {
                // sto = 100
                let coef = if current_group == 0 { 1 } else { current_group };
                current_group = coef * 100;
            }
            pos += 1;
            continue;
        }
        break;
    }

    if !consumed_any { return None; }
    total += current_group;
    Some((total, pos))
}

fn apply_cardinals_pl(text: &str) -> String { replace_numbers(text, parse_pl_number) }

regex_rules!(RE_PCT_PL, [(r"(?i)\bprocent(?:ów|ow)?\b", "%")]);
regex_rules!(RE_HOURS_PL, [
    (r"(?i)\bgodzin[aey]?\b", "h")
]);
regex_rules!(RE_CURRENCY_PL, [
    (r"(?i)\beuro\b", "\u{20AC}"),
    (r"(?i)\bdolar(?:ów|ow|y)?\b", "$"),
    (r"(?i)\bfunt(?:ów|ow|y)?\b", "\u{00A3}"),
    (r"(?i)\bzłot(?:ych|ych|y|ego)?\b", "zł"),
    (r"(?i)\bzlot(?:ych|y|ego)?\b", "zł")
]);
regex_rules!(RE_ORDINAL_PL, [
    (r"(?i)\bpierwsz[aey]\b", "1."),
    (r"(?i)\bdrug[iaey]\b", "2."),
    (r"(?i)\btrzeci[aey]?\b", "3."),
    (r"(?i)\bczwart[aey]\b", "4."),
    (r"(?i)\bpiąt[aey]\b", "5."),
    (r"(?i)\bszóst[aey]\b", "6."),
    (r"(?i)\bsiódm[aey]\b", "7."),
    (r"(?i)\bósm[aey]\b", "8."),
    (r"(?i)\bdziewiąt[aey]\b", "9."),
    (r"(?i)\bdziesiąt[aey]\b", "10.")
]);
regex_rules!(RE_UNITS_PL, [
    (r"(?i)\bkilometr(?:ów|ow|y)?\b", "km"),
    (r"(?i)\bmetr(?:ów|ow|y)?\b", "m"),
    (r"(?i)\bcentymetr(?:ów|ow|y)?\b", "cm"),
    (r"(?i)\bmilimetr(?:ów|ow|y)?\b", "mm"),
    (r"(?i)\bkilogram(?:ów|ow|y)?\b", "kg"),
    (r"(?i)\bkilo\b", "kg"),
    (r"(?i)\bgram(?:ów|ow|y)?\b", "g"),
    (r"(?i)\blitr(?:ów|ow|y)?\b", "L"),
    (r"(?i)\bstopni(?:e|a)?\b", "\u{00B0}"),
    (r"(?i)\bstopień\b", "\u{00B0}")
]);

// ---------------------------------------------------------------------------
// Russian (RU)
// ---------------------------------------------------------------------------

fn ru_atom(word: &str) -> Option<u64> {
    match word {
        "ноль" | "нуль" => Some(0),
        "один" | "одна" | "одно" => Some(1),
        "два" | "две" => Some(2),
        "три" => Some(3),
        "четыре" => Some(4),
        "пять" => Some(5),
        "шесть" => Some(6),
        "семь" => Some(7),
        "восемь" => Some(8),
        "девять" => Some(9),
        "десять" => Some(10),
        "одиннадцать" => Some(11),
        "двенадцать" => Some(12),
        "тринадцать" => Some(13),
        "четырнадцать" => Some(14),
        "пятнадцать" => Some(15),
        "шестнадцать" => Some(16),
        "семнадцать" => Some(17),
        "восемнадцать" => Some(18),
        "девятнадцать" => Some(19),
        "двадцать" => Some(20),
        "тридцать" => Some(30),
        "сорок" => Some(40),
        "пятьдесят" => Some(50),
        "шестьдесят" => Some(60),
        "семьдесят" => Some(70),
        "восемьдесят" => Some(80),
        "девяносто" => Some(90),
        _ => None,
    }
}

fn ru_multiplier(word: &str) -> Option<u64> {
    match word {
        "сто" => Some(100),
        "двести" => Some(200),
        "триста" => Some(300),
        "четыреста" => Some(400),
        "пятьсот" => Some(500),
        "шестьсот" => Some(600),
        "семьсот" => Some(700),
        "восемьсот" => Some(800),
        "девятьсот" => Some(900),
        "тысяча" | "тысячи" | "тысяч" => Some(1_000),
        "миллион" | "миллиона" | "миллионов" => Some(1_000_000),
        "миллиард" | "миллиарда" | "миллиардов" => Some(1_000_000_000),
        _ => None,
    }
}

fn parse_ru_number(words: &[&str]) -> Option<(u64, usize)> {
    if words.is_empty() { return None; }
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    let mut pos = 0;
    let mut total: u64 = 0;
    let mut current_group: u64 = 0;
    let mut consumed_any = false;

    while pos < lower.len() {
        let w = lower[pos].as_str();
        if let Some(val) = ru_atom(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }
        if let Some(mult) = ru_multiplier(w) {
            consumed_any = true;
            if (200..=900).contains(&mult) {
                total += mult;
                pos += 1;
                continue;
            }
            if mult >= 1_000_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * mult;
                current_group = 0;
            } else if mult == 1_000 {
                let coef = if current_group == 0 { 1 } else { current_group };
                total += coef * 1_000;
                current_group = 0;
            } else {
                let coef = if current_group == 0 { 1 } else { current_group };
                current_group = coef * 100;
            }
            pos += 1;
            continue;
        }
        break;
    }

    if !consumed_any { return None; }
    total += current_group;
    Some((total, pos))
}

fn apply_cardinals_ru(text: &str) -> String { replace_numbers(text, parse_ru_number) }

regex_rules!(RE_PCT_RU, [(r"(?i)\bпроцент(?:ов|а)?\b", "%")]);
regex_rules!(RE_HOURS_RU, [
    (r"(?i)\bчас(?:а|ов)?\b", "h"),
    (r"(?i)\bминут[аы]?\b", "min")
]);
regex_rules!(RE_CURRENCY_RU, [
    (r"(?i)\bевро\b", "\u{20AC}"),
    (r"(?i)\bдоллар(?:а|ов)?\b", "$"),
    (r"(?i)\bфунт(?:а|ов)?\b", "\u{00A3}"),
    (r"(?i)\bрубл(?:ь|я|ей)\b", "₽")
]);
regex_rules!(RE_ORDINAL_RU, [
    (r"(?i)\bперв(?:ый|ая|ое)\b", "1-й"),
    (r"(?i)\bвтор(?:ой|ая|ое)\b", "2-й"),
    (r"(?i)\bтрет(?:ий|ья|ье)\b", "3-й"),
    (r"(?i)\bчетвёрт(?:ый|ая|ое)\b", "4-й"),
    (r"(?i)\bпят(?:ый|ая|ое)\b", "5-й"),
    (r"(?i)\bшест(?:ой|ая|ое)\b", "6-й"),
    (r"(?i)\bседьм(?:ой|ая|ое)\b", "7-й"),
    (r"(?i)\bвосьм(?:ой|ая|ое)\b", "8-й"),
    (r"(?i)\bдевят(?:ый|ая|ое)\b", "9-й"),
    (r"(?i)\bдесят(?:ый|ая|ое)\b", "10-й")
]);
regex_rules!(RE_UNITS_RU, [
    (r"(?i)\bкилометр(?:а|ов)?\b", "km"),
    (r"(?i)\bметр(?:а|ов)?\b", "m"),
    (r"(?i)\bсантиметр(?:а|ов)?\b", "cm"),
    (r"(?i)\bмиллиметр(?:а|ов)?\b", "mm"),
    (r"(?i)\bкилограмм(?:а|ов)?\b", "kg"),
    (r"(?i)\bкило\b", "kg"),
    (r"(?i)\bграмм(?:а|ов)?\b", "g"),
    (r"(?i)\bлитр(?:а|ов)?\b", "L"),
    (r"(?i)\bградус(?:а|ов)?\b", "\u{00B0}")
]);

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
        assert_eq!(apply_itn("cinq euros", "fr-FR"), "5 \u{20AC}");
        assert_eq!(apply_itn("five dollars", "en-US"), "5 $");
    }

    // -- German --

    #[test]
    fn test_de_simple_numbers() {
        assert_eq!(apply_itn("ich habe f\u{00FC}nf Katzen", "de"), "ich habe 5 Katzen");
        assert_eq!(apply_itn("es gibt zw\u{00F6}lf Personen", "de"), "es gibt 12 Personen");
    }

    #[test]
    fn test_de_compound_numbers() {
        assert_eq!(apply_itn("dreiundzwanzig", "de"), "23");
        assert_eq!(apply_itn("einunddrei\u{00DF}ig", "de"), "31");
        assert_eq!(apply_itn("siebenundneunzig", "de"), "97");
    }

    #[test]
    fn test_de_hundreds_thousands() {
        assert_eq!(apply_itn("hundert", "de"), "100");
        assert_eq!(apply_itn("zweihundert", "de"), "200");
        assert_eq!(apply_itn("tausend", "de"), "1000");
        assert_eq!(apply_itn("zweitausend", "de"), "2000");
    }

    #[test]
    fn test_de_ordinals() {
        assert_eq!(apply_itn("der erste Januar", "de"), "der 1. Januar");
        assert_eq!(apply_itn("die dritte Runde", "de"), "die 3. Runde");
    }

    #[test]
    fn test_de_currencies() {
        assert_eq!(apply_itn("f\u{00FC}nf Euro", "de"), "5 \u{20AC}");
        assert_eq!(apply_itn("zehn Dollar", "de"), "10 $");
    }

    #[test]
    fn test_de_units() {
        assert_eq!(apply_itn("drei Kilometer", "de"), "3 km");
        assert_eq!(apply_itn("zwei Kilogramm", "de"), "2 kg");
    }

    #[test]
    fn test_de_percentages() {
        assert_eq!(apply_itn("zehn Prozent", "de"), "10 %");
    }

    // -- Spanish --

    #[test]
    fn test_es_simple_numbers() {
        assert_eq!(apply_itn("tengo cinco gatos", "es"), "tengo 5 gatos");
        assert_eq!(apply_itn("hay doce personas", "es"), "hay 12 personas");
    }

    #[test]
    fn test_es_compound_numbers() {
        assert_eq!(apply_itn("veintitr\u{00E9}s", "es"), "23");
        assert_eq!(apply_itn("treinta y uno", "es"), "31");
        assert_eq!(apply_itn("noventa y siete", "es"), "97");
    }

    #[test]
    fn test_es_hundreds() {
        assert_eq!(apply_itn("cien", "es"), "100");
        assert_eq!(apply_itn("doscientos", "es"), "200");
        assert_eq!(apply_itn("quinientos", "es"), "500");
    }

    #[test]
    fn test_es_currencies() {
        assert_eq!(apply_itn("cinco euros", "es"), "5 \u{20AC}");
    }

    #[test]
    fn test_es_ordinals() {
        assert_eq!(apply_itn("el primero de enero", "es"), "el 1.\u{00BA} de enero");
    }

    #[test]
    fn test_es_percentages() {
        assert_eq!(apply_itn("diez por ciento", "es"), "10 %");
    }

    // -- Portuguese --

    #[test]
    fn test_pt_simple_numbers() {
        assert_eq!(apply_itn("tenho cinco gatos", "pt"), "tenho 5 gatos");
        assert_eq!(apply_itn("doze pessoas", "pt"), "12 pessoas");
    }

    #[test]
    fn test_pt_compound_numbers() {
        assert_eq!(apply_itn("vinte e tr\u{00EA}s", "pt"), "23");
        assert_eq!(apply_itn("trinta e um", "pt"), "31");
    }

    #[test]
    fn test_pt_hundreds() {
        assert_eq!(apply_itn("cem", "pt"), "100");
        assert_eq!(apply_itn("duzentos", "pt"), "200");
        assert_eq!(apply_itn("quinhentos", "pt"), "500");
    }

    #[test]
    fn test_pt_currencies() {
        assert_eq!(apply_itn("cinco euros", "pt"), "5 \u{20AC}");
        assert_eq!(apply_itn("dez reais", "pt"), "10 R$");
    }

    #[test]
    fn test_pt_ordinals() {
        assert_eq!(apply_itn("o primeiro dia", "pt"), "o 1.\u{00BA} dia");
    }

    // -- Italian --

    #[test]
    fn test_it_simple_numbers() {
        assert_eq!(apply_itn("ho cinque gatti", "it"), "ho 5 gatti");
        assert_eq!(apply_itn("dodici persone", "it"), "12 persone");
    }

    #[test]
    fn test_it_compound_numbers() {
        assert_eq!(apply_itn("ventitré", "it"), "23");
        assert_eq!(apply_itn("trentuno", "it"), "31");
        assert_eq!(apply_itn("novantasette", "it"), "97");
    }

    #[test]
    fn test_it_hundreds() {
        assert_eq!(apply_itn("cento", "it"), "100");
        assert_eq!(apply_itn("duecento", "it"), "200");
    }

    #[test]
    fn test_it_currencies() {
        assert_eq!(apply_itn("cinque euro", "it"), "5 \u{20AC}");
    }

    #[test]
    fn test_it_ordinals() {
        assert_eq!(apply_itn("il primo gennaio", "it"), "il 1.\u{00BA} gennaio");
    }

    #[test]
    fn test_it_percentages() {
        assert_eq!(apply_itn("dieci per cento", "it"), "10 %");
    }

    // -- Dutch --

    #[test]
    fn test_nl_simple_numbers() {
        assert_eq!(apply_itn("ik heb vijf katten", "nl"), "ik heb 5 katten");
        assert_eq!(apply_itn("er zijn twaalf mensen", "nl"), "er zijn 12 mensen");
    }

    #[test]
    fn test_nl_compound_numbers() {
        assert_eq!(apply_itn("drieëntwintig", "nl"), "23");
        assert_eq!(apply_itn("eenendertig", "nl"), "31");
        assert_eq!(apply_itn("zevenennegen\u{0074}ig", "nl"), "97");
    }

    #[test]
    fn test_nl_hundreds() {
        assert_eq!(apply_itn("honderd", "nl"), "100");
        assert_eq!(apply_itn("tweehonderd", "nl"), "200");
    }

    #[test]
    fn test_nl_currencies() {
        assert_eq!(apply_itn("vijf euro", "nl"), "5 \u{20AC}");
    }

    #[test]
    fn test_nl_percentages() {
        assert_eq!(apply_itn("tien procent", "nl"), "10 %");
    }

    // -- Polish --

    #[test]
    fn test_pl_simple_numbers() {
        assert_eq!(apply_itn("mam pi\u{0119}\u{0107} kot\u{00F3}w", "pl"), "mam 5 kot\u{00F3}w");
        assert_eq!(apply_itn("dwana\u{015B}cie os\u{00F3}b", "pl"), "12 os\u{00F3}b");
    }

    #[test]
    fn test_pl_hundreds() {
        assert_eq!(apply_itn("sto", "pl"), "100");
        assert_eq!(apply_itn("dwie\u{015B}cie", "pl"), "200");
        assert_eq!(apply_itn("pi\u{0119}\u{0107}set", "pl"), "500");
    }

    #[test]
    fn test_pl_thousands() {
        assert_eq!(apply_itn("tysi\u{0105}c", "pl"), "1000");
        assert_eq!(apply_itn("dwa tysi\u{0105}ce", "pl"), "2000");
    }

    #[test]
    fn test_pl_currencies() {
        assert_eq!(apply_itn("pi\u{0119}\u{0107} z\u{0142}otych", "pl"), "5 z\u{0142}");
        assert_eq!(apply_itn("dziesi\u{0119}\u{0107} euro", "pl"), "10 \u{20AC}");
    }

    #[test]
    fn test_pl_ordinals() {
        assert_eq!(apply_itn("pierwszy dzie\u{0144}", "pl"), "1. dzie\u{0144}");
    }

    #[test]
    fn test_pl_percentages() {
        assert_eq!(apply_itn("dziesi\u{0119}\u{0107} procent", "pl"), "10 %");
    }

    // -- Russian --

    #[test]
    fn test_ru_simple_numbers() {
        assert_eq!(apply_itn("\u{0443} \u{043C}\u{0435}\u{043D}\u{044F} \u{043F}\u{044F}\u{0442}\u{044C} \u{043A}\u{043E}\u{0448}\u{0435}\u{043A}", "ru"),
                   "\u{0443} \u{043C}\u{0435}\u{043D}\u{044F} 5 \u{043A}\u{043E}\u{0448}\u{0435}\u{043A}");
        assert_eq!(apply_itn("\u{0434}\u{0432}\u{0435}\u{043D}\u{0430}\u{0434}\u{0446}\u{0430}\u{0442}\u{044C} \u{0447}\u{0435}\u{043B}\u{043E}\u{0432}\u{0435}\u{043A}", "ru"),
                   "12 \u{0447}\u{0435}\u{043B}\u{043E}\u{0432}\u{0435}\u{043A}");
    }

    #[test]
    fn test_ru_hundreds() {
        assert_eq!(apply_itn("\u{0441}\u{0442}\u{043E}", "ru"), "100");
        assert_eq!(apply_itn("\u{0434}\u{0432}\u{0435}\u{0441}\u{0442}\u{0438}", "ru"), "200");
        assert_eq!(apply_itn("\u{043F}\u{044F}\u{0442}\u{044C}\u{0441}\u{043E}\u{0442}", "ru"), "500");
    }

    #[test]
    fn test_ru_thousands() {
        assert_eq!(apply_itn("\u{0442}\u{044B}\u{0441}\u{044F}\u{0447}\u{0430}", "ru"), "1000");
        assert_eq!(apply_itn("\u{0434}\u{0432}\u{0435} \u{0442}\u{044B}\u{0441}\u{044F}\u{0447}\u{0438}", "ru"), "2000");
    }

    #[test]
    fn test_ru_currencies() {
        assert_eq!(apply_itn("\u{043F}\u{044F}\u{0442}\u{044C} \u{0440}\u{0443}\u{0431}\u{043B}\u{0435}\u{0439}", "ru"), "5 \u{20BD}");
        assert_eq!(apply_itn("\u{0434}\u{0435}\u{0441}\u{044F}\u{0442}\u{044C} \u{0435}\u{0432}\u{0440}\u{043E}", "ru"), "10 \u{20AC}");
    }

    #[test]
    fn test_ru_ordinals() {
        assert_eq!(apply_itn("\u{043F}\u{0435}\u{0440}\u{0432}\u{044B}\u{0439} \u{0434}\u{0435}\u{043D}\u{044C}", "ru"),
                   "1-\u{0439} \u{0434}\u{0435}\u{043D}\u{044C}");
    }

    #[test]
    fn test_ru_percentages() {
        assert_eq!(apply_itn("\u{0434}\u{0435}\u{0441}\u{044F}\u{0442}\u{044C} \u{043F}\u{0440}\u{043E}\u{0446}\u{0435}\u{043D}\u{0442}\u{043E}\u{0432}", "ru"), "10 %");
    }
}
