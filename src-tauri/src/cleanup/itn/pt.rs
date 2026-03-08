use regex::Regex;
use std::sync::LazyLock;

fn pt_atom(word: &str) -> Option<u64> {
    match word {
        "zero" => Some(0),
        "um" | "uma" => Some(1),
        "dois" | "duas" => Some(2),
        "tr\u{00EA}s" | "tres" => Some(3),
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
        "milh\u{00E3}o" | "milhao" | "milh\u{00F5}es" | "milhoes" => Some(1_000_000),
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

regex_rules!(RE_PCT_PT, [(r"(?i)\bpor cento\b", "%")]);
regex_rules!(RE_HOURS_PT, [
    (r"(?i)\bem ponto\b", ":00"),
    (r"(?i)\be meia\b", ":30"),
    (r"(?i)\bhoras?\b", "h")
]);
regex_rules!(RE_CURRENCY_PT, [
    (r"(?i)\beuros?\b", "\u{20AC}"),
    (r"(?i)\bd\u{00F3}lares?\b", "$"),
    (r"(?i)\bdolares?\b", "$"),
    (r"(?i)\blibras?\b", "\u{00A3}"),
    (r"(?i)\breais\b", "R$"),
    (r"(?i)\breal\b", "R$")
]);
regex_rules!(RE_ORDINAL_PT, [
    (r"(?i)\bprimeiro\b", "1.\u{00BA}"),
    (r"(?i)\bprimeira\b", "1.\u{00AA}"),
    (r"(?i)\bsegundo\b", "2.\u{00BA}"),
    (r"(?i)\bsegunda\b", "2.\u{00AA}"),
    (r"(?i)\bterceiro\b", "3.\u{00BA}"),
    (r"(?i)\bterceira\b", "3.\u{00AA}"),
    (r"(?i)\bquarto\b", "4.\u{00BA}"),
    (r"(?i)\bquinto\b", "5.\u{00BA}"),
    (r"(?i)\bsexto\b", "6.\u{00BA}"),
    (r"(?i)\bs\u{00E9}timo\b", "7.\u{00BA}"),
    (r"(?i)\boitavo\b", "8.\u{00BA}"),
    (r"(?i)\bnono\b", "9.\u{00BA}"),
    (r"(?i)\bd\u{00E9}cimo\b", "10.\u{00BA}")
]);
regex_rules!(RE_UNITS_PT, [
    (r"(?i)\bquil\u{00F3}metros?\b", "km"),
    (r"(?i)\bquilometros?\b", "km"),
    (r"(?i)\bmetros?\b", "m"),
    (r"(?i)\bcent\u{00ED}metros?\b", "cm"),
    (r"(?i)\bcentimetros?\b", "cm"),
    (r"(?i)\bquilogramas?\b", "kg"),
    (r"(?i)\bquilos?\b", "kg"),
    (r"(?i)\bgramas?\b", "g"),
    (r"(?i)\blitros?\b", "L"),
    (r"(?i)\bgraus?\b", "\u{00B0}")
]);

pub(super) fn apply_all(text: &str) -> String {
    let mut r = super::apply_regex_list(text, &RE_PCT_PT);
    r = super::apply_regex_list(&r, &RE_HOURS_PT);
    r = super::apply_regex_list(&r, &RE_CURRENCY_PT);
    r = super::apply_regex_list(&r, &RE_ORDINAL_PT);
    r = super::apply_regex_list(&r, &RE_UNITS_PT);
    super::replace_numbers(&r, parse_pt_number)
}

#[cfg(test)]
mod tests {
    use crate::cleanup::itn::apply_itn;

    #[test]
    fn simple_numbers() {
        assert_eq!(apply_itn("tenho cinco gatos", "pt"), "tenho 5 gatos");
        assert_eq!(apply_itn("doze pessoas", "pt"), "12 pessoas");
    }

    #[test]
    fn compound_numbers() {
        assert_eq!(apply_itn("vinte e tr\u{00EA}s", "pt"), "23");
        assert_eq!(apply_itn("trinta e um", "pt"), "31");
    }

    #[test]
    fn hundreds() {
        assert_eq!(apply_itn("cem", "pt"), "100");
        assert_eq!(apply_itn("duzentos", "pt"), "200");
        assert_eq!(apply_itn("quinhentos", "pt"), "500");
    }

    #[test]
    fn currencies() {
        assert_eq!(apply_itn("cinco euros", "pt"), "5 \u{20AC}");
        assert_eq!(apply_itn("dez reais", "pt"), "10 R$");
    }

    #[test]
    fn ordinals() {
        assert_eq!(apply_itn("o primeiro dia", "pt"), "o 1.\u{00BA} dia");
    }
}
