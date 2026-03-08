use regex::Regex;
use std::sync::LazyLock;

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

        if w == "and" {
            if consumed_any && pos + 1 < lower.len() {
                pos += 1;
                continue;
            }
            break;
        }

        if w.contains('-') {
            let parts: Vec<&str> = w.split('-').collect();
            if let Some((val, _)) = parse_en_number(&parts) {
                current_group += val;
                pos += 1;
                consumed_any = true;
                continue;
            }
        }

        if w == "a" && pos + 1 < lower.len() && en_multiplier(lower[pos + 1].as_str()).is_some() {
            current_group += 1;
            pos += 1;
            consumed_any = true;
            continue;
        }

        if let Some(val) = en_atom(w) {
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

static RE_PCT_EN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bpercent\b").unwrap()
});

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

static RE_CURRENCY_EN: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"(?i)\bdollars?\b").unwrap(), "$"),
        (Regex::new(r"(?i)\beuros?\b").unwrap(), "\u{20AC}"),
        (Regex::new(r"(?i)\bpounds?\b").unwrap(), "\u{00A3}"),
    ]
});

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

pub(super) fn apply_all(text: &str) -> String {
    let mut r = RE_PCT_EN.replace_all(text, "%").to_string();
    r = super::apply_regex_list(&r, &RE_HOURS_EN);
    r = super::apply_regex_list(&r, &RE_CURRENCY_EN);
    r = super::apply_regex_list(&r, &RE_ORDINAL_EN);
    r = super::apply_regex_list(&r, &RE_UNITS_EN);
    super::replace_numbers(&r, parse_en_number)
}

#[cfg(test)]
mod tests {
    use crate::cleanup::itn::apply_itn;

    #[test]
    fn simple_numbers() {
        assert_eq!(apply_itn("I have five cats", "en"), "I have 5 cats");
        assert_eq!(apply_itn("there are twelve people", "en"), "there are 12 people");
    }

    #[test]
    fn compound_numbers() {
        assert_eq!(apply_itn("twenty-three", "en"), "23");
        assert_eq!(apply_itn("twenty three", "en"), "23");
        assert_eq!(apply_itn("ninety seven", "en"), "97");
    }

    #[test]
    fn hundreds() {
        assert_eq!(apply_itn("one hundred", "en"), "100");
        assert_eq!(apply_itn("two hundred and fifty", "en"), "250");
        assert_eq!(apply_itn("three hundred twenty one", "en"), "321");
    }

    #[test]
    fn thousands() {
        assert_eq!(apply_itn("one thousand", "en"), "1000");
        assert_eq!(apply_itn("two thousand", "en"), "2000");
        assert_eq!(apply_itn("three thousand two hundred", "en"), "3200");
    }

    #[test]
    fn ordinals() {
        assert_eq!(apply_itn("the first of January", "en"), "the 1st of January");
        assert_eq!(apply_itn("the third time", "en"), "the 3rd time");
    }

    #[test]
    fn percentages() {
        assert_eq!(apply_itn("ten percent", "en"), "10 %");
    }

    #[test]
    fn hours() {
        assert_eq!(apply_itn("three o'clock", "en"), "3 :00");
    }

    #[test]
    fn currencies() {
        assert_eq!(apply_itn("five dollars", "en"), "5 $");
    }

    #[test]
    fn units() {
        assert_eq!(apply_itn("two kilometers", "en"), "2 km");
        assert_eq!(apply_itn("five miles", "en"), "5 mi");
    }

    #[test]
    fn mixed() {
        assert_eq!(
            apply_itn("I am twenty three years old and weigh one hundred fifty pounds", "en"),
            "I am 23 years old and weigh 150 \u{00A3}"
        );
    }

    #[test]
    fn million() {
        assert_eq!(apply_itn("three million", "en"), "3000000");
    }

    #[test]
    fn complex_number() {
        assert_eq!(apply_itn("one thousand two hundred and thirty four", "en"), "1234");
    }

    #[test]
    fn degrees() {
        assert_eq!(apply_itn("seventy degrees", "en"), "70 \u{00B0}");
    }
}
