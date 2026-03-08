use regex::Regex;
use std::sync::LazyLock;

fn it_atom(word: &str) -> Option<u64> {
    match word {
        "zero" => Some(0),
        "uno" | "una" | "un" => Some(1),
        "due" => Some(2),
        "tre" | "tr\u{00E9}" => Some(3),
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

/// Decompose Italian compound numbers: "ventitr\u{00E9}" -> 23, "trentuno" -> 31, "duecento" -> 200
fn it_decompose_compound(word: &str) -> Option<u64> {
    // Compound tens: ventitr\u{00E9}, trentuno, novantasette
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

    if !consumed_any {
        return None;
    }
    total += current_group;
    Some((total, pos))
}

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
    (r"(?i)\bprimo\b", "1.\u{00BA}"),
    (r"(?i)\bprima\b", "1.\u{00AA}"),
    (r"(?i)\bsecondo\b", "2.\u{00BA}"),
    (r"(?i)\bseconda\b", "2.\u{00AA}"),
    (r"(?i)\bterzo\b", "3.\u{00BA}"),
    (r"(?i)\bterza\b", "3.\u{00AA}"),
    (r"(?i)\bquarto\b", "4.\u{00BA}"),
    (r"(?i)\bquinto\b", "5.\u{00BA}"),
    (r"(?i)\bsesto\b", "6.\u{00BA}"),
    (r"(?i)\bsettimo\b", "7.\u{00BA}"),
    (r"(?i)\bottavo\b", "8.\u{00BA}"),
    (r"(?i)\bnono\b", "9.\u{00BA}"),
    (r"(?i)\bdecimo\b", "10.\u{00BA}")
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

pub(super) fn apply_all(text: &str) -> String {
    let mut r = super::apply_regex_list(text, &RE_PCT_IT);
    r = super::apply_regex_list(&r, &RE_HOURS_IT);
    r = super::apply_regex_list(&r, &RE_CURRENCY_IT);
    r = super::apply_regex_list(&r, &RE_ORDINAL_IT);
    r = super::apply_regex_list(&r, &RE_UNITS_IT);
    super::replace_numbers(&r, parse_it_number)
}

#[cfg(test)]
mod tests {
    use crate::cleanup::itn::apply_itn;

    #[test]
    fn simple_numbers() {
        assert_eq!(apply_itn("ho cinque gatti", "it"), "ho 5 gatti");
        assert_eq!(apply_itn("dodici persone", "it"), "12 persone");
    }

    #[test]
    fn compound_numbers() {
        assert_eq!(apply_itn("ventitr\u{00E9}", "it"), "23");
        assert_eq!(apply_itn("trentuno", "it"), "31");
        assert_eq!(apply_itn("novantasette", "it"), "97");
    }

    #[test]
    fn hundreds() {
        assert_eq!(apply_itn("cento", "it"), "100");
        assert_eq!(apply_itn("duecento", "it"), "200");
    }

    #[test]
    fn currencies() {
        assert_eq!(apply_itn("cinque euro", "it"), "5 \u{20AC}");
    }

    #[test]
    fn ordinals() {
        assert_eq!(apply_itn("il primo gennaio", "it"), "il 1.\u{00BA} gennaio");
    }

    #[test]
    fn percentages() {
        assert_eq!(apply_itn("dieci per cento", "it"), "10 %");
    }
}
