use regex::Regex;

pub fn process(text: &str, language: &str) -> String {
    let mut result = text.to_string();

    let lang = resolve_language(language, &result);
    result = apply_dictation_commands(&result, &lang);
    result = fix_punctuation_spacing(&result);
    result = capitalize_after_sentence_endings(&result);

    // Capitalize first character
    if let Some(first) = result.chars().next() {
        if first.is_lowercase() {
            let upper: String = first.to_uppercase().collect();
            result = format!("{}{}", upper, &result[first.len_utf8()..]);
        }
    }

    result
}

fn resolve_language(code: &str, text: &str) -> String {
    if code != "auto" {
        return code.to_string();
    }

    let french_words: &[&str] = &[
        "le", "la", "les", "de", "des", "du", "un", "une", "et", "est",
        "que", "qui", "dans", "pour", "pas", "sur", "avec", "tout", "mais", "comme",
    ];

    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let french_count = words.iter().filter(|w| french_words.contains(w)).count();

    if french_count >= 2 { "fr".to_string() } else { "en".to_string() }
}

fn apply_dictation_commands(text: &str, language: &str) -> String {
    let commands: Vec<(&str, &str)> = if language.starts_with("fr") {
        vec![
            ("point d'interrogation", "?"),
            ("point d'exclamation", "!"),
            ("points de suspension", "\u{2026}"),
            ("point-virgule", ";"),
            ("point virgule", ";"),
            ("deux-points", ":"),
            ("deux points", ":"),
            ("ouvrir la parenthèse", "("),
            ("fermer la parenthèse", ")"),
            ("ouvrir les guillemets", "«\u{00A0}"),
            ("fermer les guillemets", "\u{00A0}»"),
            ("à la ligne", "\n"),
            ("nouvelle ligne", "\n"),
            ("nouveau paragraphe", "\n\n"),
            ("virgule", ","),
            ("point", "."),
            ("tiret", "-"),
        ]
    } else {
        vec![
            ("question mark", "?"),
            ("exclamation mark", "!"),
            ("exclamation point", "!"),
            ("semicolon", ";"),
            ("semi-colon", ";"),
            ("ellipsis", "\u{2026}"),
            ("colon", ":"),
            ("open parenthesis", "("),
            ("close parenthesis", ")"),
            ("open paren", "("),
            ("close paren", ")"),
            ("open quote", "\""),
            ("close quote", "\""),
            ("new line", "\n"),
            ("newline", "\n"),
            ("new paragraph", "\n\n"),
            ("comma", ","),
            ("period", "."),
            ("full stop", "."),
            ("dash", "-"),
            ("hyphen", "-"),
        ]
    };

    let mut result = text.to_string();
    for (pattern, replacement) in commands {
        // Case-insensitive replacement
        let re = Regex::new(&format!("(?i){}", regex::escape(pattern))).unwrap();
        result = re.replace_all(&result, replacement).to_string();
    }
    result
}

fn fix_punctuation_spacing(text: &str) -> String {
    let mut result = text.to_string();

    // Remove space before closing punctuation: "word ." → "word."
    let re1 = Regex::new(r#"\s+([.,?!;:\u{2026})\u{00BB}"\]])"#).unwrap();
    result = re1.replace_all(&result, "$1").to_string();

    // Ensure space after punctuation (except before newline, end, or more punctuation)
    let re2 = Regex::new(r#"([.,?!;:\u{2026}])([^\s\n.,?!;:\u{2026})\u{00BB}"\]\d])"#).unwrap();
    result = re2.replace_all(&result, "$1 $2").to_string();

    // Remove space after opening punctuation: "( word" → "(word"
    let re3 = Regex::new(r#"([\(\u{00AB}"\[])\s+"#).unwrap();
    result = re3.replace_all(&result, "$1").to_string();

    result
}

fn capitalize_after_sentence_endings(text: &str) -> String {
    let re = Regex::new(r"([.?!]\s+|\n)(\p{Ll})").unwrap();
    let result = re.replace_all(text, |caps: &regex::Captures| {
        let prefix = &caps[1];
        let letter = &caps[2];
        format!("{}{}", prefix, letter.to_uppercase())
    });
    result.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_french_commands() {
        let result = process("bonjour virgule comment allez-vous point d'interrogation", "fr");
        assert_eq!(result, "Bonjour, comment allez-vous?");
    }

    #[test]
    fn test_english_commands() {
        let result = process("hello comma how are you question mark", "en");
        assert_eq!(result, "Hello, how are you?");
    }

    #[test]
    fn test_auto_detect_french() {
        let result = process("le chat est dans la maison", "auto");
        assert!(result.starts_with("Le"));
    }

    #[test]
    fn test_capitalization() {
        let result = process("hello. world", "en");
        assert_eq!(result, "Hello. World");
    }
}
