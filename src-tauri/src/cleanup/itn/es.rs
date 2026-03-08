use regex::Regex;
use std::sync::LazyLock;

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

        if w == "y" {
            if consumed_any && pos + 1 < lower.len() {
                pos += 1;
                continue;
            }
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

    if !consumed_any {
        return None;
    }
    total += current_group;
    Some((total, pos))
}

regex_rules!(RE_PCT_ES, [(r"(?i)\bpor ciento\b", "%")]);
regex_rules!(RE_HOURS_ES, [
    (r"(?i)\ben punto\b", ":00"),
    (r"(?i)\by media\b", ":30"),
    (r"(?i)\by cuarto\b", ":15"),
    (r"(?i)\bmenos cuarto\b", ":45")
]);
regex_rules!(RE_CURRENCY_ES, [
    (r"(?i)\beuros?\b", "\u{20AC}"),
    (r"(?i)\bd\u{00F3}lares?\b", "$"),
    (r"(?i)\bdolares?\b", "$"),
    (r"(?i)\blibras?\b", "\u{00A3}"),
    (r"(?i)\bpesos?\b", "$")
]);
regex_rules!(RE_ORDINAL_ES, [
    (r"(?i)\bprimero\b", "1.\u{00BA}"),
    (r"(?i)\bprimera\b", "1.\u{00AA}"),
    (r"(?i)\bsegundo\b", "2.\u{00BA}"),
    (r"(?i)\bsegunda\b", "2.\u{00AA}"),
    (r"(?i)\btercero\b", "3.\u{00BA}"),
    (r"(?i)\btercera\b", "3.\u{00AA}"),
    (r"(?i)\bcuarto\b", "4.\u{00BA}"),
    (r"(?i)\bquinto\b", "5.\u{00BA}"),
    (r"(?i)\bsexto\b", "6.\u{00BA}"),
    (r"(?i)\bs\u{00E9}ptimo\b", "7.\u{00BA}"),
    (r"(?i)\boctavo\b", "8.\u{00BA}"),
    (r"(?i)\bnoveno\b", "9.\u{00BA}"),
    (r"(?i)\bd\u{00E9}cimo\b", "10.\u{00BA}")
]);
regex_rules!(RE_UNITS_ES, [
    (r"(?i)\bkil\u{00F3}metros?\b", "km"),
    (r"(?i)\bkilometros?\b", "km"),
    (r"(?i)\bmetros?\b", "m"),
    (r"(?i)\bcent\u{00ED}metros?\b", "cm"),
    (r"(?i)\bcentimetros?\b", "cm"),
    (r"(?i)\bmil\u{00ED}metros?\b", "mm"),
    (r"(?i)\bmilimetros?\b", "mm"),
    (r"(?i)\bkilogramos?\b", "kg"),
    (r"(?i)\bkilos?\b", "kg"),
    (r"(?i)\bgramos?\b", "g"),
    (r"(?i)\blitros?\b", "L"),
    (r"(?i)\bgrados?\b", "\u{00B0}")
]);

pub(super) fn apply_all(text: &str) -> String {
    let mut r = super::apply_regex_list(text, &RE_PCT_ES);
    r = super::apply_regex_list(&r, &RE_HOURS_ES);
    r = super::apply_regex_list(&r, &RE_CURRENCY_ES);
    r = super::apply_regex_list(&r, &RE_ORDINAL_ES);
    r = super::apply_regex_list(&r, &RE_UNITS_ES);
    super::replace_numbers(&r, parse_es_number)
}

#[cfg(test)]
mod tests {
    use crate::cleanup::itn::apply_itn;

    #[test]
    fn simple_numbers() {
        assert_eq!(apply_itn("tengo cinco gatos", "es"), "tengo 5 gatos");
        assert_eq!(apply_itn("hay doce personas", "es"), "hay 12 personas");
    }

    #[test]
    fn compound_numbers() {
        assert_eq!(apply_itn("veintitr\u{00E9}s", "es"), "23");
        assert_eq!(apply_itn("treinta y uno", "es"), "31");
        assert_eq!(apply_itn("noventa y siete", "es"), "97");
    }

    #[test]
    fn hundreds() {
        assert_eq!(apply_itn("cien", "es"), "100");
        assert_eq!(apply_itn("doscientos", "es"), "200");
        assert_eq!(apply_itn("quinientos", "es"), "500");
    }

    #[test]
    fn currencies() {
        assert_eq!(apply_itn("cinco euros", "es"), "5 \u{20AC}");
    }

    #[test]
    fn ordinals() {
        assert_eq!(apply_itn("el primero de enero", "es"), "el 1.\u{00BA} de enero");
    }

    #[test]
    fn percentages() {
        assert_eq!(apply_itn("diez por ciento", "es"), "10 %");
    }
}
