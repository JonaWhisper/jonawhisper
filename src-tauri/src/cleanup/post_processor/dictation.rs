use regex::Regex;
use std::sync::LazyLock;

static DICTATION_COMMANDS_FR: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        ("point d'interrogation", "?"),
        ("point d'exclamation", "!"),
        ("points de suspension", "\u{2026}"),
        ("point-virgule", ";"),
        ("point virgule", ";"),
        ("deux-points", ":"),
        ("deux points", ":"),
        ("ouvrir la parenth\u{00E8}se", "("),
        ("fermer la parenth\u{00E8}se", ")"),
        ("ouvrir les guillemets", "\u{00AB}\u{00A0}"),
        ("fermer les guillemets", "\u{00A0}\u{00BB}"),
        ("\u{00E0} la ligne", "\n"),
        ("nouvelle ligne", "\n"),
        ("nouveau paragraphe", "\n\n"),
        ("virgule", ","),
        ("point", "."),
        ("tiret", "-"),
    ]
    .iter()
    .map(|(p, r)| (Regex::new(&format!("(?i){}", regex::escape(p))).unwrap(), *r))
    .collect()
});

static DICTATION_COMMANDS_EN: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
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
    .iter()
    .map(|(p, r)| (Regex::new(&format!("(?i){}", regex::escape(p))).unwrap(), *r))
    .collect()
});

pub(super) fn apply_dictation_commands(text: &str, language: &str) -> String {
    let commands = if language.starts_with("fr") {
        &*DICTATION_COMMANDS_FR
    } else {
        &*DICTATION_COMMANDS_EN
    };

    let mut result = text.to_string();
    for (re, replacement) in commands {
        result = re.replace_all(&result, *replacement).to_string();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: apply_dictation_commands does raw regex replacement.
    // Surrounding spaces are preserved — finalize() in the pipeline handles spacing.

    // -- French dictation commands --

    #[test]
    fn fr_virgule() {
        let result = apply_dictation_commands("bonjour virgule monde", "fr");
        assert!(result.contains(','), "Expected comma in: {:?}", result);
        assert!(result.contains("bonjour"));
        assert!(result.contains("monde"));
    }

    #[test]
    fn fr_point_interrogation() {
        let result = apply_dictation_commands("comment allez-vous point d'interrogation", "fr");
        assert!(result.contains('?'), "Expected ? in: {:?}", result);
    }

    #[test]
    fn fr_point_exclamation() {
        let result = apply_dictation_commands("super point d'exclamation", "fr");
        assert!(result.contains('!'), "Expected ! in: {:?}", result);
    }

    #[test]
    fn fr_nouvelle_ligne() {
        let result = apply_dictation_commands("bonjour \u{00E0} la ligne monde", "fr");
        assert!(result.contains('\n'), "Expected newline in: {:?}", result);
    }

    #[test]
    fn fr_nouveau_paragraphe() {
        let result = apply_dictation_commands("fin nouveau paragraphe d\u{00E9}but", "fr");
        assert!(result.contains("\n\n"), "Expected double newline in: {:?}", result);
    }

    #[test]
    fn fr_guillemets() {
        let result = apply_dictation_commands(
            "il a dit ouvrir les guillemets bonjour fermer les guillemets", "fr"
        );
        assert!(result.contains("\u{00AB}"), "Expected \u{00AB} in: {:?}", result);
        assert!(result.contains("\u{00BB}"), "Expected \u{00BB} in: {:?}", result);
    }

    #[test]
    fn fr_points_de_suspension() {
        let result = apply_dictation_commands("alors points de suspension", "fr");
        assert!(result.contains('\u{2026}'), "Expected ellipsis in: {:?}", result);
    }

    #[test]
    fn fr_deux_points() {
        let result = apply_dictation_commands("voici deux points la suite", "fr");
        assert!(result.contains(':'), "Expected colon in: {:?}", result);
    }

    #[test]
    fn fr_tiret() {
        let result = apply_dictation_commands("peut tiret \u{00EA}tre", "fr");
        assert!(result.contains('-'), "Expected dash in: {:?}", result);
    }

    // -- English dictation commands --

    #[test]
    fn en_comma() {
        let result = apply_dictation_commands("hello comma world", "en");
        assert!(result.contains(','), "Expected comma in: {:?}", result);
        assert!(!result.to_lowercase().contains("comma"));
    }

    #[test]
    fn en_question_mark() {
        let result = apply_dictation_commands("how are you question mark", "en");
        assert!(result.contains('?'), "Expected ? in: {:?}", result);
        assert!(!result.to_lowercase().contains("question mark"));
    }

    #[test]
    fn en_exclamation() {
        let result = apply_dictation_commands("wow exclamation mark", "en");
        assert!(result.contains('!'), "Expected ! in: {:?}", result);

        let result2 = apply_dictation_commands("wow exclamation point", "en");
        assert!(result2.contains('!'), "Expected ! in: {:?}", result2);
    }

    #[test]
    fn en_period() {
        let result = apply_dictation_commands("done period", "en");
        assert!(result.contains('.'), "Expected period in: {:?}", result);

        let result2 = apply_dictation_commands("done full stop", "en");
        assert!(result2.contains('.'), "Expected period in: {:?}", result2);
    }

    #[test]
    fn en_new_line() {
        let result = apply_dictation_commands("hello new line world", "en");
        assert!(result.contains('\n'), "Expected newline in: {:?}", result);
    }

    #[test]
    fn en_parentheses() {
        let result = apply_dictation_commands("open parenthesis note close parenthesis", "en");
        assert!(result.contains('('), "Expected ( in: {:?}", result);
        assert!(result.contains(')'), "Expected ) in: {:?}", result);
        assert!(result.contains("note"));
    }

    #[test]
    fn en_semicolon() {
        let result = apply_dictation_commands("first semicolon second", "en");
        assert!(result.contains(';'), "Expected ; in: {:?}", result);
    }

    // -- Case insensitive --

    #[test]
    fn case_insensitive() {
        let result = apply_dictation_commands("hello COMMA world", "en");
        assert!(result.contains(','), "Expected comma in: {:?}", result);

        let result = apply_dictation_commands("bonjour VIRGULE monde", "fr");
        assert!(result.contains(','), "Expected comma in: {:?}", result);
    }

    // -- Language routing --

    #[test]
    fn fr_prefix_uses_french_commands() {
        let result = apply_dictation_commands("bonjour virgule monde", "fr-CA");
        assert!(result.contains(','), "Expected comma in: {:?}", result);
    }

    #[test]
    fn non_french_uses_english_commands() {
        let result = apply_dictation_commands("hello comma world", "de");
        assert!(result.contains(','), "Expected comma in: {:?}", result);
    }

    // -- No match passthrough --

    #[test]
    fn no_commands_unchanged() {
        assert_eq!(apply_dictation_commands("just normal text", "en"), "just normal text");
    }

    // -- Multiple commands in one sentence --

    #[test]
    fn multiple_commands() {
        let result = apply_dictation_commands("hello comma how are you question mark", "en");
        assert!(result.contains(','), "Expected comma in: {:?}", result);
        assert!(result.contains('?'), "Expected ? in: {:?}", result);
        assert!(!result.to_lowercase().contains("comma"));
        assert!(!result.to_lowercase().contains("question mark"));
    }
}
