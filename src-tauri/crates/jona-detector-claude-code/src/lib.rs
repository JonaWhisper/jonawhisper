//! Detect Claude Code OAuth token from macOS Keychain.
//!
//! Claude Code stores credentials in a Keychain entry:
//! - Service: "Claude Code-credentials"
//! - Account: macOS username
//! - Value: JSON with `{"claudeAiOauth":{"accessToken":"sk-ant-oat01-..."}}`
//!
//! The token is tied to the user's Claude subscription (Pro/Max/Team).
//! It expires ~8h and is refreshed by Claude Code.

use jona_types::provider::{DetectedCredential, DetectorRegistration};

const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

fn detect() -> Vec<DetectedCredential> {
    let username = whoami::username();
    let entry = match keyring::Entry::new(KEYCHAIN_SERVICE, &username) {
        Ok(e) => e,
        Err(e) => {
            log::debug!("claude-code detector: keyring entry error: {e}");
            return vec![];
        }
    };

    let json_str = match entry.get_password() {
        Ok(s) => s,
        Err(keyring::Error::NoEntry) => return vec![],
        Err(e) => {
            log::debug!("claude-code detector: keyring read error: {e}");
            return vec![];
        }
    };

    let token = match extract_token(&json_str) {
        Some(t) => t,
        None => return vec![],
    };

    log::info!(
        "claude-code detector: found OAuth token ({}...)",
        &token[..token.len().min(12)]
    );

    vec![DetectedCredential {
        kind: "anthropic",
        source_label: "Claude Code",
        api_key: token,
        url: String::new(),
        extra: std::collections::HashMap::new(),
    }]
}

fn extract_token(json_str: &str) -> Option<String> {
    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("claude-code detector: invalid JSON: {e}");
            return None;
        }
    };

    let token = parsed
        .get("claudeAiOauth")
        .and_then(|o| o.get("accessToken"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if token.is_empty() {
        log::debug!("claude-code detector: no accessToken found");
        return None;
    }

    Some(token.to_string())
}

inventory::submit! {
    DetectorRegistration {
        id: "claude-code",
        display_name: "Claude Code",
        detect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_claude_code_json() {
        let json = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-test123","refreshToken":"rt-test"}}"#;
        let token = extract_token(json).unwrap();
        assert_eq!(token, "sk-ant-oat01-test123");
    }

    #[test]
    fn parse_missing_oauth_key() {
        let json = r#"{"someOtherField": true}"#;
        assert!(extract_token(json).is_none());
    }

    #[test]
    fn parse_empty_access_token() {
        let json = r#"{"claudeAiOauth":{"accessToken":""}}"#;
        assert!(extract_token(json).is_none());
    }

    #[test]
    fn parse_invalid_json() {
        assert!(extract_token("not json").is_none());
    }

    #[test]
    fn detector_registration_well_formed() {
        let reg = DetectorRegistration {
            id: "claude-code",
            display_name: "Claude Code",
            detect,
        };
        assert_eq!(reg.id, "claude-code");
    }
}
