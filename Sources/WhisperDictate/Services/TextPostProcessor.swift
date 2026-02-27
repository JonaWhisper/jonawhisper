import Foundation

struct TextPostProcessor {
    static func process(_ text: String, language: String) -> String {
        var result = text

        let lang = resolveLanguage(language, text: text)
        result = applyDictationCommands(result, language: lang)
        result = fixPunctuationSpacing(result)
        result = capitalizeAfterSentenceEndings(result)

        // Capitalize first character
        if let first = result.first, first.isLowercase {
            result = first.uppercased() + result.dropFirst()
        }

        return result
    }

    // MARK: - Language detection

    private static func resolveLanguage(_ code: String, text: String) -> String {
        if code != "auto" { return code }
        let frenchWords: Set = ["le", "la", "les", "de", "des", "du", "un", "une", "et", "est",
                                "que", "qui", "dans", "pour", "pas", "sur", "avec", "tout", "mais", "comme"]
        let words = text.lowercased().split(separator: " ").map(String.init)
        let frenchCount = words.filter { frenchWords.contains($0) }.count
        return frenchCount >= 2 ? "fr" : "en"
    }

    // MARK: - Dictation commands

    private static func applyDictationCommands(_ text: String, language: String) -> String {
        let commands: [(pattern: String, replacement: String)]

        if language.hasPrefix("fr") {
            commands = [
                ("point d'interrogation", "?"),
                ("point d'exclamation", "!"),
                ("points de suspension", "…"),
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
            commands = [
                ("question mark", "?"),
                ("exclamation mark", "!"),
                ("exclamation point", "!"),
                ("semicolon", ";"),
                ("semi-colon", ";"),
                ("ellipsis", "…"),
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
        }

        var result = text
        for cmd in commands {
            result = result.replacingOccurrences(
                of: cmd.pattern,
                with: cmd.replacement,
                options: [.caseInsensitive]
            )
        }
        return result
    }

    // MARK: - Punctuation spacing

    private static func fixPunctuationSpacing(_ text: String) -> String {
        var result = text

        // Remove space before closing punctuation: "word ." → "word."
        result = result.replacingOccurrences(
            of: "\\s+([.,?!;:…)»\"\\]])",
            with: "$1",
            options: .regularExpression
        )

        // Ensure space after punctuation (except before newline, end, or more punctuation)
        result = result.replacingOccurrences(
            of: "([.,?!;:…])([^\\s\\n.,?!;:…)»\"\\]\\d])",
            with: "$1 $2",
            options: .regularExpression
        )

        // Remove space after opening punctuation: "( word" → "(word"
        result = result.replacingOccurrences(
            of: "([(«\"\\[])\\s+",
            with: "$1",
            options: .regularExpression
        )

        return result
    }

    // MARK: - Capitalization

    private static func capitalizeAfterSentenceEndings(_ text: String) -> String {
        var result = text
        let pattern = "([.?!]\\s+|\\n)(\\p{Ll})"
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return result }

        let nsString = result as NSString
        let matches = regex.matches(in: result, range: NSRange(location: 0, length: nsString.length))

        for match in matches.reversed() {
            let letterRange = match.range(at: 2)
            let letter = nsString.substring(with: letterRange)
            result = (result as NSString).replacingCharacters(in: letterRange, with: letter.uppercased())
        }

        return result
    }
}
