//! Detect Claude Code OAuth token from macOS Keychain.
//!
//! Claude Code stores credentials in a Keychain entry:
//! - Service: "Claude Code-credentials"
//! - Account: macOS username
//! - Value: JSON with `{"claudeAiOauth":{"accessToken":"sk-ant-oat01-...","expiresAt":...}}`
//!
//! The token is tied to the user's Claude subscription (Pro/Max/Team).
//! It expires ~8h and is refreshed by Claude Code.
//!
//! This detector caches the token internally and only re-reads the Keychain
//! when the token has expired (based on `expiresAt` from the JSON).

use jona_types::provider::{DetectedCredential, DetectorRegistration};
use std::sync::Mutex;

const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

struct CachedToken {
    token: String,
    expires_at_ms: u64,
}

static CACHE: Mutex<Option<CachedToken>> = Mutex::new(None);

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn detect() -> Vec<DetectedCredential> {
    // Check cache first
    {
        let cache = CACHE.lock().unwrap();
        if let Some(ref cached) = *cache {
            if now_ms() < cached.expires_at_ms {
                log::debug!("claude-code detector: using cached token (expires in {}s)",
                    (cached.expires_at_ms - now_ms()) / 1000);
                return vec![DetectedCredential {
                    kind: "anthropic",
                    source_label: "Claude Code",
                    api_key: cached.token.clone(),
                    url: String::new(),
                    extra: std::collections::HashMap::new(),
                }];
            }
            log::debug!("claude-code detector: cached token expired, re-reading Keychain");
        }
    }

    // Cache miss or expired — read from Keychain
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

    let (token, expires_at_ms) = match extract_token_and_expiry(&json_str) {
        Some(t) => t,
        None => return vec![],
    };

    log::info!(
        "claude-code detector: found OAuth token ({}...), expires in {}s",
        &token[..token.len().min(12)],
        expires_at_ms.saturating_sub(now_ms()) / 1000
    );

    // Update cache
    *CACHE.lock().unwrap() = Some(CachedToken {
        token: token.clone(),
        expires_at_ms,
    });

    vec![DetectedCredential {
        kind: "anthropic",
        source_label: "Claude Code",
        api_key: token,
        url: String::new(),
        extra: std::collections::HashMap::new(),
    }]
}

fn extract_token_and_expiry(json_str: &str) -> Option<(String, u64)> {
    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("claude-code detector: invalid JSON: {e}");
            return None;
        }
    };

    let oauth = parsed.get("claudeAiOauth")?;

    let token = oauth.get("accessToken")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if token.is_empty() {
        log::debug!("claude-code detector: no accessToken found");
        return None;
    }

    let expires_at_ms = oauth.get("expiresAt")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    Some((token.to_string(), expires_at_ms))
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
    fn parse_valid_with_expiry() {
        let json = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-test123","refreshToken":"rt-test","expiresAt":1773372454773}}"#;
        let (token, expires) = extract_token_and_expiry(json).unwrap();
        assert_eq!(token, "sk-ant-oat01-test123");
        assert_eq!(expires, 1773372454773);
    }

    #[test]
    fn parse_valid_without_expiry() {
        let json = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-test123"}}"#;
        let (token, expires) = extract_token_and_expiry(json).unwrap();
        assert_eq!(token, "sk-ant-oat01-test123");
        assert_eq!(expires, 0);
    }

    #[test]
    fn parse_missing_oauth_key() {
        let json = r#"{"someOtherField": true}"#;
        assert!(extract_token_and_expiry(json).is_none());
    }

    #[test]
    fn parse_empty_access_token() {
        let json = r#"{"claudeAiOauth":{"accessToken":""}}"#;
        assert!(extract_token_and_expiry(json).is_none());
    }

    #[test]
    fn parse_invalid_json() {
        assert!(extract_token_and_expiry("not json").is_none());
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
