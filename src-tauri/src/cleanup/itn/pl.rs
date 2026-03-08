use regex::Regex;
use std::sync::LazyLock;

fn pl_atom(word: &str) -> Option<u64> {
    match word {
        "zero" => Some(0),
        "jeden" | "jedna" | "jedno" => Some(1),
        "dwa" | "dwie" => Some(2),
        "trzy" => Some(3),
        "cztery" => Some(4),
        "pi\u{0119}\u{0107}" | "piec" => Some(5),
        "sze\u{015B}\u{0107}" | "szesc" => Some(6),
        "siedem" => Some(7),
        "osiem" => Some(8),
        "dziewi\u{0119}\u{0107}" | "dziewiec" => Some(9),
        "dziesi\u{0119}\u{0107}" | "dziesiec" => Some(10),
        "jedena\u{015B}cie" | "jedenascie" => Some(11),
        "dwana\u{015B}cie" | "dwanascie" => Some(12),
        "trzyna\u{015B}cie" | "trzynascie" => Some(13),
        "czterna\u{015B}cie" | "czternascie" => Some(14),
        "pi\u{0119}tna\u{015B}cie" | "pietnascie" => Some(15),
        "szesna\u{015B}cie" | "szesnascie" => Some(16),
        "siedemna\u{015B}cie" | "siedemnascie" => Some(17),
        "osiemna\u{015B}cie" | "osiemnascie" => Some(18),
        "dziewi\u{0119}tna\u{015B}cie" | "dziewietnascie" => Some(19),
        "dwadzie\u{015B}cia" | "dwadziescia" => Some(20),
        "trzydzie\u{015B}ci" | "trzydziesci" => Some(30),
        "czterdzieści" | "czterdziesci" => Some(40),
        "pi\u{0119}\u{0107}dziesi\u{0105}t" | "piecdziesiat" => Some(50),
        "sze\u{015B}\u{0107}dziesi\u{0105}t" | "szescdziesiat" => Some(60),
        "siedemdziesi\u{0105}t" | "siedemdziesiat" => Some(70),
        "osiemdziesi\u{0105}t" | "osiemdziesiat" => Some(80),
        "dziewi\u{0119}\u{0107}dziesi\u{0105}t" | "dziewiecdziesiat" => Some(90),
        _ => None,
    }
}

fn pl_multiplier(word: &str) -> Option<u64> {
    match word {
        "sto" => Some(100),
        "dwie\u{015B}cie" | "dwiescie" => Some(200),
        "trzysta" => Some(300),
        "czterysta" => Some(400),
        "pi\u{0119}\u{0107}set" | "piecset" => Some(500),
        "sze\u{015B}\u{0107}set" | "szescset" => Some(600),
        "siedemset" => Some(700),
        "osiemset" => Some(800),
        "dziewi\u{0119}\u{0107}set" | "dziewiecset" => Some(900),
        "tysi\u{0105}c" | "tysiac" | "tysi\u{0105}ce" | "tysiace" | "tysi\u{0119}cy" | "tysiecy" => Some(1_000),
        "milion" | "miliony" | "milion\u{00F3}w" | "milionow" => Some(1_000_000),
        "miliard" | "miliardy" | "miliard\u{00F3}w" | "miliardow" => Some(1_000_000_000),
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

regex_rules!(RE_PCT_PL, [(r"(?i)\bprocent(?:\u{00F3}w|ow)?\b", "%")]);
regex_rules!(RE_HOURS_PL, [
    (r"(?i)\bgodzin[aey]?\b", "h")
]);
regex_rules!(RE_CURRENCY_PL, [
    (r"(?i)\beuro\b", "\u{20AC}"),
    (r"(?i)\bdolar(?:\u{00F3}w|ow|y)?\b", "$"),
    (r"(?i)\bfunt(?:\u{00F3}w|ow|y)?\b", "\u{00A3}"),
    (r"(?i)\bz\u{0142}ot(?:ych|ych|y|ego)?\b", "z\u{0142}"),
    (r"(?i)\bzlot(?:ych|y|ego)?\b", "z\u{0142}")
]);
regex_rules!(RE_ORDINAL_PL, [
    (r"(?i)\bpierwsz[aey]\b", "1."),
    (r"(?i)\bdrug[iaey]\b", "2."),
    (r"(?i)\btrzeci[aey]?\b", "3."),
    (r"(?i)\bczwart[aey]\b", "4."),
    (r"(?i)\bpi\u{0105}t[aey]\b", "5."),
    (r"(?i)\bsz\u{00F3}st[aey]\b", "6."),
    (r"(?i)\bsi\u{00F3}dm[aey]\b", "7."),
    (r"(?i)\b\u{00F3}sm[aey]\b", "8."),
    (r"(?i)\bdziewi\u{0105}t[aey]\b", "9."),
    (r"(?i)\bdziesi\u{0105}t[aey]\b", "10.")
]);
regex_rules!(RE_UNITS_PL, [
    (r"(?i)\bkilometr(?:\u{00F3}w|ow|y)?\b", "km"),
    (r"(?i)\bmetr(?:\u{00F3}w|ow|y)?\b", "m"),
    (r"(?i)\bcentymetr(?:\u{00F3}w|ow|y)?\b", "cm"),
    (r"(?i)\bmilimetr(?:\u{00F3}w|ow|y)?\b", "mm"),
    (r"(?i)\bkilogram(?:\u{00F3}w|ow|y)?\b", "kg"),
    (r"(?i)\bkilo\b", "kg"),
    (r"(?i)\bgram(?:\u{00F3}w|ow|y)?\b", "g"),
    (r"(?i)\blitr(?:\u{00F3}w|ow|y)?\b", "L"),
    (r"(?i)\bstopni(?:e|a)?\b", "\u{00B0}"),
    (r"(?i)\bstopie\u{0144}\b", "\u{00B0}")
]);

const UNITS_PL: &[&str] = &[
    "godzina", "godziny", "godzin", "minuta", "minuty", "minut",
    "sekunda", "sekundy", "sekund", "euro", "dolar", "funt",
    "kilogram", "gram", "litr", "metr", "kilometr", "centymetr", "milimetr",
    "stopie\u{0144}", "stopni", "procent",
];

pub(super) fn apply_all(text: &str) -> String {
    let mut r = super::apply_regex_list(text, &RE_PCT_PL);
    r = super::apply_regex_list(&r, &RE_HOURS_PL);
    r = super::apply_regex_list(&r, &RE_CURRENCY_PL);
    r = super::apply_regex_list(&r, &RE_ORDINAL_PL);
    r = super::apply_regex_list(&r, &RE_UNITS_PL);
    super::replace_numbers(&r, parse_pl_number, UNITS_PL)
}

#[cfg(test)]
mod tests {
    use crate::cleanup::itn::apply_itn;

    #[test]
    fn simple_numbers() {
        assert_eq!(apply_itn("mam pi\u{0119}\u{0107} kot\u{00F3}w", "pl"), "mam 5 kot\u{00F3}w");
        assert_eq!(apply_itn("dwana\u{015B}cie os\u{00F3}b", "pl"), "12 os\u{00F3}b");
    }

    #[test]
    fn hundreds() {
        assert_eq!(apply_itn("sto", "pl"), "100");
        assert_eq!(apply_itn("dwie\u{015B}cie", "pl"), "200");
        assert_eq!(apply_itn("pi\u{0119}\u{0107}set", "pl"), "500");
    }

    #[test]
    fn thousands() {
        assert_eq!(apply_itn("tysi\u{0105}c", "pl"), "1000");
        assert_eq!(apply_itn("dwa tysi\u{0105}ce", "pl"), "2000");
    }

    #[test]
    fn currencies() {
        assert_eq!(apply_itn("pi\u{0119}\u{0107} z\u{0142}otych", "pl"), "5 z\u{0142}");
        assert_eq!(apply_itn("dziesi\u{0119}\u{0107} euro", "pl"), "10 \u{20AC}");
    }

    #[test]
    fn ordinals() {
        assert_eq!(apply_itn("pierwszy dzie\u{0144}", "pl"), "1. dzie\u{0144}");
    }

    #[test]
    fn percentages() {
        assert_eq!(apply_itn("dziesi\u{0119}\u{0107} procent", "pl"), "10 %");
    }
}
