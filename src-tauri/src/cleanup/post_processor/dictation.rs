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
