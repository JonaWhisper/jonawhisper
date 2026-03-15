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
//! ## Two-phase detection
//!
//! 1. **Probe** (`detect()`): checks if the Keychain entry *exists* via
//!    `SecItemCopyMatching` with attributes-only (no `kSecReturnData`).
//!    This does NOT trigger a macOS authorization popup. Returns a
//!    `DetectedCredential` with an empty `api_key`.
//!
//! 2. **Read** (`refresh_credential`): called when the provider is actually
//!    used for an API call. Reads the secret from the Keychain (may prompt
//!    once for authorization) and caches the token until it expires.
//!
//! This crate is macOS-only: it is listed under `[target.'cfg(target_os = "macos")'.dependencies]`
//! in the main Cargo.toml and is not compiled on other platforms.

use jona_types::provider::{DetectedCredential, DetectorRegistration};

#[cfg(target_os = "macos")]
mod keychain {
    use jona_types::provider::DetectedCredential;
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

    /// Check if the Keychain entry exists WITHOUT reading the secret.
    /// Uses `SecItemCopyMatching` with attributes-only — no authorization popup.
    fn entry_exists() -> bool {
        use core_foundation::base::{CFType, TCFType};
        use core_foundation::boolean::CFBoolean;
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::string::CFString;

        let username = whoami::username();

        let keys = vec![
            unsafe { CFString::wrap_under_get_rule(security_framework_sys::item::kSecClass) },
            unsafe { CFString::wrap_under_get_rule(security_framework_sys::item::kSecAttrService) },
            unsafe { CFString::wrap_under_get_rule(security_framework_sys::item::kSecAttrAccount) },
            unsafe { CFString::wrap_under_get_rule(security_framework_sys::item::kSecReturnAttributes) },
        ];
        let values: Vec<CFType> = vec![
            unsafe { CFType::wrap_under_get_rule(security_framework_sys::item::kSecClassGenericPassword as *const _) },
            CFString::new(KEYCHAIN_SERVICE).as_CFType(),
            CFString::new(&username).as_CFType(),
            CFBoolean::true_value().as_CFType(),
        ];

        let query = CFDictionary::from_CFType_pairs(&keys.iter().zip(values.iter()).map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>());

        let mut result = std::ptr::null();
        let status = unsafe {
            security_framework_sys::keychain_item::SecItemCopyMatching(
                query.as_concrete_TypeRef(),
                &mut result,
            )
        };

        if !result.is_null() {
            unsafe { core_foundation::base::CFRelease(result) };
        }

        status == 0 // errSecSuccess
    }

    /// Probe-only: check if Claude Code credentials exist in the Keychain.
    /// Returns a credential with empty api_key (token is read lazily via refresh).
    pub(crate) fn detect() -> Vec<DetectedCredential> {
        // If we have a valid cached token, return it directly
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
            }
        }

        // Probe without reading the secret — no popup
        if entry_exists() {
            log::debug!("claude-code detector: Keychain entry exists (probe, no secret read)");
            vec![DetectedCredential {
                kind: "anthropic",
                source_label: "Claude Code",
                api_key: String::new(), // empty = not yet read
                url: String::new(),
                extra: std::collections::HashMap::new(),
            }]
        } else {
            log::debug!("claude-code detector: no Keychain entry found");
            vec![]
        }
    }

    /// Actually read the token from Keychain (called via refresh_credential).
    /// This MAY trigger a macOS authorization popup on first access.
    pub(crate) fn read_token() -> Vec<DetectedCredential> {
        // Check cache first
        {
            let cache = CACHE.lock().unwrap();
            if let Some(ref cached) = *cache {
                if now_ms() < cached.expires_at_ms {
                    return vec![DetectedCredential {
                        kind: "anthropic",
                        source_label: "Claude Code",
                        api_key: cached.token.clone(),
                        url: String::new(),
                        extra: std::collections::HashMap::new(),
                    }];
                }
            }
        }

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

        let (token, expires_at_ms) = match super::extract_token_and_expiry(&json_str) {
            Some(t) => t,
            None => return vec![],
        };

        log::debug!(
            "claude-code detector: read OAuth token, expires in {}s",
            expires_at_ms.saturating_sub(now_ms()) / 1000
        );

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
}

fn detect() -> Vec<DetectedCredential> {
    #[cfg(target_os = "macos")]
    { keychain::detect() }
    #[cfg(not(target_os = "macos"))]
    { vec![] }
}

fn refresh() -> Vec<DetectedCredential> {
    #[cfg(target_os = "macos")]
    { keychain::read_token() }
    #[cfg(not(target_os = "macos"))]
    { vec![] }
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

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let expires_at_ms = oauth.get("expiresAt")
        .and_then(|v| v.as_u64())
        .filter(|&t| t > 0)
        .unwrap_or(now + 3_600_000); // 1h fallback if missing/zero

    Some((token.to_string(), expires_at_ms))
}

inventory::submit! {
    DetectorRegistration {
        id: "claude-code",
        display_name: "Claude Code",
        detect,
        refresh: Some(refresh),
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
    fn parse_valid_without_expiry_uses_fallback() {
        let json = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-test123"}}"#;
        let (token, expires) = extract_token_and_expiry(json).unwrap();
        assert_eq!(token, "sk-ant-oat01-test123");
        // Fallback: ~1h from now (not 0)
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
        assert!(expires > now_ms);
        assert!(expires <= now_ms + 3_600_100); // 1h + small margin
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
            refresh: Some(refresh),
        };
        assert_eq!(reg.id, "claude-code");
    }
}
