use regex::Regex;
use std::sync::LazyLock;

fn nl_atom(word: &str) -> Option<u64> {
    match word {
        "nul" => Some(0),
        "een" | "\u{00E9}\u{00E9}n" => Some(1),
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

/// Decompose Dutch compound numbers: "drie\u{00EB}ntwintig" -> 23, "tweehonderd" -> 200
fn nl_decompose_compound(word: &str) -> Option<u64> {
    // Dutch uses "\u{00EB}n" or "en" as connector: drie\u{00EB}ntwintig, eenendertig
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
    // Compound hundreds: tweehonderd -> 200
    if let Some(prefix) = word.strip_suffix("honderd") {
        if !prefix.is_empty() {
            return nl_atom(prefix).map(|n| n * 100);
        }
    }
    // Compound thousands: tweeduizend -> 2000
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

const UNITS_NL: &[&str] = &[
    "uur", "minuut", "minuten", "seconde", "seconden",
    "euro", "dollar", "pond", "kilo", "kilogram", "gram",
    "liter", "meter", "kilometer", "centimeter", "millimeter",
    "graad", "graden", "procent",
];

pub(super) fn apply_all(text: &str) -> String {
    let mut r = super::apply_regex_list(text, &RE_PCT_NL);
    r = super::apply_regex_list(&r, &RE_HOURS_NL);
    r = super::apply_regex_list(&r, &RE_CURRENCY_NL);
    r = super::apply_regex_list(&r, &RE_ORDINAL_NL);
    r = super::apply_regex_list(&r, &RE_UNITS_NL);
    super::replace_numbers(&r, parse_nl_number, UNITS_NL)
}

#[cfg(test)]
mod tests {
    use crate::cleanup::itn::apply_itn;

    #[test]
    fn simple_numbers() {
        assert_eq!(apply_itn("ik heb vijf katten", "nl"), "ik heb 5 katten");
        assert_eq!(apply_itn("er zijn twaalf mensen", "nl"), "er zijn 12 mensen");
    }

    #[test]
    fn compound_numbers() {
        assert_eq!(apply_itn("drie\u{00EB}ntwintig", "nl"), "23");
        assert_eq!(apply_itn("eenendertig", "nl"), "31");
        assert_eq!(apply_itn("zevenennegentig", "nl"), "97");
    }

    #[test]
    fn hundreds() {
        assert_eq!(apply_itn("honderd", "nl"), "100");
        assert_eq!(apply_itn("tweehonderd", "nl"), "200");
    }

    #[test]
    fn currencies() {
        assert_eq!(apply_itn("vijf euro", "nl"), "5 \u{20AC}");
    }

    #[test]
    fn percentages() {
        assert_eq!(apply_itn("tien procent", "nl"), "10 %");
    }
}
