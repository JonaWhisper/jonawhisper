use regex::Regex;
use std::sync::LazyLock;

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

/// Try to decompose a German compound number word: "dreiundzwanzig" -> 23, "zweihundert" -> 200
fn de_decompose_compound(word: &str) -> Option<u64> {
    // Split on "und" — unit+und+tens: "dreiundzwanzig" -> 23
    if let Some(pos) = word.find("und") {
        let left = &word[..pos];
        let right = &word[pos + 3..];
        if let (Some(unit), Some(tens)) = (de_atom(left), de_atom(right)) {
            if unit < 10 && (20..=90).contains(&tens) && tens % 10 == 0 {
                return Some(tens + unit);
            }
        }
    }
    // "zweihundert" -> 200, "dreihundert" -> 300
    if let Some(prefix) = word.strip_suffix("hundert") {
        if !prefix.is_empty() {
            return de_atom(prefix).map(|n| n * 100);
        }
    }
    // "zweitausend" -> 2000
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

    if !consumed_any {
        return None;
    }
    total += current_group;
    Some((total, pos))
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

const UNITS_DE: &[&str] = &[
    "stunde", "stunden", "minute", "minuten", "sekunde", "sekunden",
    "euro", "dollar", "pfund", "kilo", "kilogramm", "gramm",
    "liter", "meter", "kilometer", "zentimeter", "millimeter",
    "grad", "prozent",
];

pub(super) fn apply_all(text: &str) -> String {
    let mut r = super::apply_regex_list(text, &RE_PCT_DE);
    r = super::apply_regex_list(&r, &RE_HOURS_DE);
    r = super::apply_regex_list(&r, &RE_CURRENCY_DE);
    r = super::apply_regex_list(&r, &RE_ORDINAL_DE);
    r = super::apply_regex_list(&r, &RE_UNITS_DE);
    super::replace_numbers(&r, parse_de_number, UNITS_DE)
}

#[cfg(test)]
mod tests {
    use crate::cleanup::itn::apply_itn;

    #[test]
    fn simple_numbers() {
        assert_eq!(apply_itn("ich habe f\u{00FC}nf Katzen", "de"), "ich habe 5 Katzen");
        assert_eq!(apply_itn("es gibt zw\u{00F6}lf Personen", "de"), "es gibt 12 Personen");
    }

    #[test]
    fn compound_numbers() {
        assert_eq!(apply_itn("dreiundzwanzig", "de"), "23");
        assert_eq!(apply_itn("einunddrei\u{00DF}ig", "de"), "31");
        assert_eq!(apply_itn("siebenundneunzig", "de"), "97");
    }

    #[test]
    fn hundreds_thousands() {
        assert_eq!(apply_itn("hundert", "de"), "100");
        assert_eq!(apply_itn("zweihundert", "de"), "200");
        assert_eq!(apply_itn("tausend", "de"), "1000");
        assert_eq!(apply_itn("zweitausend", "de"), "2000");
    }

    #[test]
    fn ordinals() {
        assert_eq!(apply_itn("der erste Januar", "de"), "der 1. Januar");
        assert_eq!(apply_itn("die dritte Runde", "de"), "die 3. Runde");
    }

    #[test]
    fn currencies() {
        assert_eq!(apply_itn("f\u{00FC}nf Euro", "de"), "5 \u{20AC}");
        assert_eq!(apply_itn("zehn Dollar", "de"), "10 $");
    }

    #[test]
    fn units() {
        assert_eq!(apply_itn("drei Kilometer", "de"), "3 km");
        assert_eq!(apply_itn("zwei Kilogramm", "de"), "2 kg");
    }

    #[test]
    fn percentages() {
        assert_eq!(apply_itn("zehn Prozent", "de"), "10 %");
    }
}
