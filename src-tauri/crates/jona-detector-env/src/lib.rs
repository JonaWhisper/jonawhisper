//! Detects API keys from environment variables.

use jona_types::DetectedCredential;

/// Static mapping: (env var name, preset ID, source label).
const ENV_MAPPINGS: &[(&str, &str, &str)] = &[
    ("OPENAI_API_KEY",     "openai",     "env:OPENAI_API_KEY"),
    ("ANTHROPIC_API_KEY",  "anthropic",  "env:ANTHROPIC_API_KEY"),
    ("GROQ_API_KEY",       "groq",       "env:GROQ_API_KEY"),
    ("GEMINI_API_KEY",     "gemini",     "env:GEMINI_API_KEY"),
    ("MISTRAL_API_KEY",    "mistral",    "env:MISTRAL_API_KEY"),
    ("DEEPSEEK_API_KEY",   "deepseek",   "env:DEEPSEEK_API_KEY"),
    ("TOGETHER_API_KEY",   "together",   "env:TOGETHER_API_KEY"),
    ("FIREWORKS_API_KEY",  "fireworks",  "env:FIREWORKS_API_KEY"),
    ("CEREBRAS_API_KEY",   "cerebras",   "env:CEREBRAS_API_KEY"),
    ("OPENROUTER_API_KEY",        "openrouter", "env:OPENROUTER_API_KEY"),
    ("CLAUDE_CODE_OAUTH_TOKEN",   "anthropic",  "env:CLAUDE_CODE_OAUTH_TOKEN"),
];

/// Testable detection from an arbitrary list of (var_name, value) pairs.
fn detect_from(vars: &[(&str, &str)]) -> Vec<DetectedCredential> {
    let mut results = Vec::new();
    for &(env_var, preset_id, source_label) in ENV_MAPPINGS {
        if let Some(&(_, value)) = vars.iter().find(|&&(k, _)| k == env_var) {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                log::debug!("env detector: {} is empty, skipping", env_var);
                continue;
            }
            log::debug!("env detector: found {}", env_var);
            results.push(DetectedCredential {
                kind: preset_id,
                source_label,
                api_key: trimmed.to_string(),
                url: String::new(),
                extra: std::collections::HashMap::new(),
            });
        }
    }
    results
}

/// Read real environment and detect credentials.
fn detect() -> Vec<DetectedCredential> {
    let vars: Vec<(&str, String)> = ENV_MAPPINGS
        .iter()
        .filter_map(|&(var, _, _)| std::env::var(var).ok().map(|v| (var, v)))
        .collect();
    let pairs: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
    detect_from(&pairs)
}

inventory::submit! {
    jona_types::DetectorRegistration {
        id: "env",
        display_name: "Environment Variables",
        detect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_from_finds_set_vars() {
        let vars = vec![
            ("OPENAI_API_KEY", "sk-test-123"),
            ("GROQ_API_KEY", "gsk-test-456"),
        ];
        let results = detect_from(&vars);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].kind, "openai");
        assert_eq!(results[0].api_key, "sk-test-123");
        assert_eq!(results[0].source_label, "env:OPENAI_API_KEY");
        assert_eq!(results[1].kind, "groq");
        assert_eq!(results[1].api_key, "gsk-test-456");
    }

    #[test]
    fn detect_from_skips_empty_and_whitespace() {
        let vars = vec![
            ("OPENAI_API_KEY", ""),
            ("GROQ_API_KEY", "   "),
            ("MISTRAL_API_KEY", "valid-key"),
        ];
        let results = detect_from(&vars);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "mistral");
    }

    #[test]
    fn detect_from_skips_unknown_vars() {
        let vars = vec![("UNKNOWN_API_KEY", "some-value")];
        let results = detect_from(&vars);
        assert!(results.is_empty());
    }

    #[test]
    fn all_mappings_produce_correct_preset_ids() {
        let vars: Vec<(&str, &str)> = ENV_MAPPINGS
            .iter()
            .map(|&(var, _, _)| (var, "test-key"))
            .collect();
        let results = detect_from(&vars);
        assert_eq!(results.len(), ENV_MAPPINGS.len());
        for (result, &(_, expected_kind, expected_label)) in
            results.iter().zip(ENV_MAPPINGS.iter())
        {
            assert_eq!(result.kind, expected_kind);
            assert_eq!(result.source_label, expected_label);
            assert_eq!(result.api_key, "test-key");
        }
    }
}
