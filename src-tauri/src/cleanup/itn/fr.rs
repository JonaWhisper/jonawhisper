use regex::Regex;
use std::sync::LazyLock;

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

        if w == "et" {
            if consumed_any && pos + 1 < lower.len() {
                pos += 1;
                continue;
            }
            break;
        }

        if w == "quatre" && pos + 1 < lower.len() && (lower[pos + 1] == "vingt" || lower[pos + 1] == "vingts") {
            current_group += 80;
            pos += 2;
            consumed_any = true;
            continue;
        }

        if w.contains('-') {
            let parts: Vec<&str> = w.split('-').collect();
            if let Some((val, _)) = parse_fr_number(&parts) {
                current_group += val;
                pos += 1;
                consumed_any = true;
                continue;
            }
        }

        if let Some(val) = fr_atom(w) {
            current_group += val;
            pos += 1;
            consumed_any = true;
            continue;
        }

        if let Some(mult) = fr_multiplier(w) {
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

static RE_PCT_FR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bpour cent\b").unwrap()
});

static RE_HOURS_FR: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\bet quart\b").unwrap(), "15"),
        (Regex::new(r"(?i)\bet demie?\b").unwrap(), "30"),
        (Regex::new(r"(?i)\bmoins le quart\b").unwrap(), "45"),
    ]
});

static RE_HEURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\d)\s*heures?\b").unwrap()
});

static RE_CURRENCY_FR: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\beuros?\b").unwrap(), "\u{20AC}"),
        (Regex::new(r"(?i)\bdollars?\b").unwrap(), "$"),
        (Regex::new(r"(?i)\blivres? sterling\b").unwrap(), "\u{00A3}"),
        (Regex::new(r"(?i)\blivres?\b").unwrap(), "\u{00A3}"),
    ]
});

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

/// French unit words for "un/une" disambiguation.
const UNITS_FR: &[&str] = &[
    "heure", "heures", "minute", "minutes", "seconde", "secondes",
    "euro", "euros", "dollar", "dollars", "livre", "livres",
    "kilo", "kilos", "kilogramme", "kilogrammes", "gramme", "grammes",
    "litre", "litres", "millilitre", "millilitres",
    "m\u{00e8}tre", "m\u{00e8}tres", "kilom\u{00e8}tre", "kilom\u{00e8}tres",
    "centim\u{00e8}tre", "centim\u{00e8}tres", "millim\u{00e8}tre", "millim\u{00e8}tres",
    "degr\u{00e9}", "degr\u{00e9}s", "pourcent",
];

pub(super) fn apply_all(text: &str) -> String {
    let mut r = RE_PCT_FR.replace_all(text, "%").to_string();
    // Currencies
    r = super::apply_regex_list(&r, &RE_CURRENCY_FR);
    // Ordinals
    r = super::apply_regex_list(&r, &RE_ORDINAL_FR);
    // Units
    r = super::apply_regex_list(&r, &RE_UNITS_FR);
    // Cardinals (must run before hours so "trois heures" → "3 heures")
    r = super::replace_numbers(&r, parse_fr_number, UNITS_FR);
    // Hours (after number conversion: "3 heures" → "3 h", but not standalone "heure")
    for (re, replacement) in RE_HOURS_FR.iter() {
        r = re.replace_all(&r, *replacement).to_string();
    }
    r = RE_HEURE.replace_all(&r, "${1} h").to_string();
    r
}

#[cfg(test)]
mod tests {
    use crate::cleanup::itn::apply_itn;

    #[test]
    fn simple_numbers() {
        assert_eq!(apply_itn("j'ai cinq chats", "fr"), "j'ai 5 chats");
        assert_eq!(apply_itn("il y a douze personnes", "fr"), "il y a 12 personnes");
        assert_eq!(apply_itn("seize ans", "fr"), "16 ans");
    }

    #[test]
    fn compound_numbers() {
        assert_eq!(apply_itn("vingt-trois", "fr"), "23");
        assert_eq!(apply_itn("vingt et un", "fr"), "21");
        assert_eq!(apply_itn("soixante-dix", "fr"), "70");
        assert_eq!(apply_itn("quatre-vingt-dix-sept", "fr"), "97");
        assert_eq!(apply_itn("quatre-vingts", "fr"), "80");
    }

    #[test]
    fn hundreds() {
        assert_eq!(apply_itn("cent", "fr"), "100");
        assert_eq!(apply_itn("deux cents", "fr"), "200");
        assert_eq!(apply_itn("trois cent vingt-et-un", "fr"), "321");
    }

    #[test]
    fn thousands() {
        assert_eq!(apply_itn("mille", "fr"), "1000");
        assert_eq!(apply_itn("deux mille", "fr"), "2000");
        assert_eq!(apply_itn("trois mille deux cents", "fr"), "3200");
    }

    #[test]
    fn standalone_un_not_converted() {
        assert_eq!(apply_itn("un chat", "fr"), "un chat");
    }

    #[test]
    fn ordinals() {
        assert_eq!(apply_itn("le premier janvier", "fr"), "le 1er janvier");
        assert_eq!(apply_itn("la deuxième fois", "fr"), "la 2e fois");
    }

    #[test]
    fn percentages() {
        assert_eq!(apply_itn("dix pour cent", "fr"), "10 %");
    }

    #[test]
    fn hours() {
        assert_eq!(apply_itn("trois heures", "fr"), "3 h");
        assert_eq!(apply_itn("trois heures et quart", "fr"), "3 h 15");
        assert_eq!(apply_itn("trois heures et demie", "fr"), "3 h 30");
    }

    #[test]
    fn currencies() {
        assert_eq!(apply_itn("cinq euros", "fr"), "5 \u{20AC}");
    }

    #[test]
    fn units() {
        assert_eq!(apply_itn("deux kilomètres", "fr"), "2 km");
        assert_eq!(apply_itn("trois kilos", "fr"), "3 kg");
    }

    #[test]
    fn mixed() {
        assert_eq!(
            apply_itn("j'ai vingt-trois ans et je fais soixante-dix kilos", "fr"),
            "j'ai 23 ans et je fais 70 kg"
        );
    }

    #[test]
    fn million() {
        assert_eq!(apply_itn("deux millions", "fr"), "2000000");
    }

    #[test]
    fn complex_number() {
        assert_eq!(apply_itn("mille deux cent trente-quatre", "fr"), "1234");
    }

    #[test]
    fn degrees() {
        assert_eq!(apply_itn("vingt degrés", "fr"), "20 \u{00B0}");
    }

    #[test]
    fn standalone_heure_not_converted() {
        // "heure" without a number before it should NOT be converted to "h"
        assert_eq!(apply_itn("on va parler en heure", "fr"), "on va parler en heure");
        assert_eq!(apply_itn("c'est l'heure de partir", "fr"), "c'est l'heure de partir");
        // "une" before a unit IS converted to "1"
        assert_eq!(apply_itn("une heure de vol", "fr"), "1 h de vol");
        // "une" before a non-unit stays as article
        assert_eq!(apply_itn("une pomme", "fr"), "une pomme");
    }

    #[test]
    fn hours_with_digits() {
        // Numbers already as digits should still work
        assert_eq!(apply_itn("5 heures", "fr"), "5 h");
        assert_eq!(apply_itn("14 heures 30", "fr"), "14 h 30");
    }

    #[test]
    fn hours_complex() {
        assert_eq!(apply_itn("quatorze heures trente", "fr"), "14 h 30");
        assert_eq!(apply_itn("le rendez-vous est à trois heures", "fr"), "le rendez-vous est à 3 h");
    }
}
