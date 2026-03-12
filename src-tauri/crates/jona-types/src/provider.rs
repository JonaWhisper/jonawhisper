//! Cloud provider types, trait, and auto-registration.

use crate::{Provider, TranscriptionResult};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

/// Type of form field for a preset's extra parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Password,
    Select,
    Toggle,
}

/// A custom field defined by a provider preset.
#[derive(Debug, Clone)]
pub struct PresetField {
    /// Unique field identifier (e.g. "region", "access_key").
    pub id: &'static str,
    /// Display label (English fallback — frontend uses i18n key `provider.field.{id}`).
    pub label: &'static str,
    /// Field type controls rendering and masking behavior.
    pub field_type: FieldType,
    /// Whether the field must be non-empty to save.
    pub required: bool,
    /// Placeholder text shown in the input.
    pub placeholder: &'static str,
    /// Default value (empty string if none).
    pub default_value: &'static str,
    /// For Select fields: available options as `(value, label)` pairs.
    pub options: &'static [(&'static str, &'static str)],
    /// Whether the value is sensitive (stored in keychain, masked in IPC).
    pub sensitive: bool,
}

/// Errors returned by cloud provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error: HTTP {status}: {body}")]
    Api { status: u16, body: String },
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Not configured: {0}")]
    NotConfigured(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Object-safe trait implemented by each cloud provider backend.
pub trait CloudProvider: Send + Sync {
    /// Transcribe audio via cloud ASR (blocking).
    fn transcribe(
        &self,
        provider: &Provider,
        model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError>;

    /// Chat completion for text cleanup (async).
    fn chat_completion<'a>(
        &'a self,
        provider: &'a Provider,
        model: &'a str,
        system: &'a str,
        user_message: &'a str,
        temperature: f32,
        max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + 'a>>;

    /// List available models (async).
    fn list_models<'a>(
        &'a self,
        provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>>;
}

/// Auto-registration entry for cloud provider backends (one per backend_id).
pub struct ProviderRegistration {
    pub backend_id: &'static str,
    pub factory: fn() -> Box<dyn CloudProvider>,
}

inventory::collect!(ProviderRegistration);

/// Data-driven preset for a known cloud provider.
/// Registered via `inventory::submit!` in provider crates.
pub struct ProviderPreset {
    pub id: &'static str,
    pub display_name: &'static str,
    pub base_url: &'static str,
    pub backend_id: &'static str,
    pub supports_asr: bool,
    pub supports_llm: bool,
    pub gradient: &'static str,
    pub default_asr_models: &'static [&'static str],
    pub default_llm_models: &'static [&'static str],
    /// Additional fields this preset needs beyond name/apiKey/url.
    pub extra_fields: &'static [PresetField],
    /// Default field IDs to hide (e.g. `&["base_url"]` for providers with fixed endpoints).
    pub hidden_fields: &'static [&'static str],
}

inventory::collect!(ProviderPreset);

/// Look up a preset by ID from the inventory-collected presets.
/// This is a simple linear scan suitable for infrequent calls (e.g. save/load).
pub fn preset_by_id(id: &str) -> Option<&'static ProviderPreset> {
    inventory::iter::<ProviderPreset>.into_iter().find(|p| p.id == id)
}

/// Parse model IDs from an OpenAI-compatible JSON response.
/// Handles both `{data:[...]}` and bare `[...]` formats.
pub fn parse_model_ids_from_json(json: &serde_json::Value) -> Result<Vec<String>, ProviderError> {
    let models_array = json
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| json.as_array());

    let ids: Vec<String> = models_array
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if ids.is_empty() {
        return Err(ProviderError::InvalidResponse(
            "No models found in response".into(),
        ));
    }

    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_model_ids_from_json --

    #[test]
    fn parse_openai_wrapped_format() {
        let json = serde_json::json!({
            "data": [
                {"id": "gpt-4o", "object": "model"},
                {"id": "gpt-4o-mini", "object": "model"},
            ]
        });
        let ids = parse_model_ids_from_json(&json).unwrap();
        assert_eq!(ids, vec!["gpt-4o", "gpt-4o-mini"]);
    }

    #[test]
    fn parse_bare_array_format() {
        let json = serde_json::json!([
            {"id": "llama-3.1-8b"},
            {"id": "whisper-large-v3"},
        ]);
        let ids = parse_model_ids_from_json(&json).unwrap();
        assert_eq!(ids, vec!["llama-3.1-8b", "whisper-large-v3"]);
    }

    #[test]
    fn parse_empty_data_array_returns_error() {
        let json = serde_json::json!({"data": []});
        assert!(parse_model_ids_from_json(&json).is_err());
    }

    #[test]
    fn parse_empty_bare_array_returns_error() {
        let json = serde_json::json!([]);
        assert!(parse_model_ids_from_json(&json).is_err());
    }

    #[test]
    fn parse_no_id_fields_returns_error() {
        let json = serde_json::json!({
            "data": [{"name": "model1"}, {"name": "model2"}]
        });
        assert!(parse_model_ids_from_json(&json).is_err());
    }

    #[test]
    fn parse_mixed_entries_skips_missing_ids() {
        let json = serde_json::json!({
            "data": [
                {"id": "valid-model"},
                {"name": "no-id-here"},
                {"id": "another-model"},
            ]
        });
        let ids = parse_model_ids_from_json(&json).unwrap();
        assert_eq!(ids, vec!["valid-model", "another-model"]);
    }

    #[test]
    fn parse_non_string_id_skipped() {
        let json = serde_json::json!({
            "data": [{"id": 42}, {"id": "valid"}]
        });
        let ids = parse_model_ids_from_json(&json).unwrap();
        assert_eq!(ids, vec!["valid"]);
    }

    #[test]
    fn parse_plain_object_returns_error() {
        let json = serde_json::json!({"error": "not found"});
        assert!(parse_model_ids_from_json(&json).is_err());
    }

    #[test]
    fn parse_data_takes_precedence_over_root_array() {
        // If both "data" key and root array exist (unlikely but possible),
        // "data" should take precedence per OpenAI convention
        let json = serde_json::json!({"data": [{"id": "from-data"}]});
        let ids = parse_model_ids_from_json(&json).unwrap();
        assert_eq!(ids, vec!["from-data"]);
    }

    // -- ProviderError --

    #[test]
    fn provider_error_display() {
        let e = ProviderError::Http("connection refused".into());
        assert!(e.to_string().contains("connection refused"));

        let e = ProviderError::Api { status: 429, body: "rate limited".into() };
        assert!(e.to_string().contains("429"));
        assert!(e.to_string().contains("rate limited"));

        let e = ProviderError::NotConfigured("no key".into());
        assert!(e.to_string().contains("no key"));

        let e = ProviderError::InvalidResponse("bad json".into());
        assert!(e.to_string().contains("bad json"));
    }

    // -- ProviderPreset --

    #[test]
    fn provider_preset_inventory_collects() {
        // Verify the inventory collection macro compiles and the type is usable
        let preset = ProviderPreset {
            id: "test",
            display_name: "Test",
            base_url: "https://example.com",
            backend_id: "openai",
            supports_asr: true,
            supports_llm: true,
            gradient: "none",
            default_asr_models: &["model-1"],
            default_llm_models: &[],
            extra_fields: &[],
            hidden_fields: &[],
        };
        assert_eq!(preset.id, "test");
        assert_eq!(preset.backend_id, "openai");
        assert!(preset.supports_asr);
        assert!(preset.default_asr_models.len() == 1);
    }

    // -- ProviderRegistration --

    #[test]
    fn provider_registration_backend_id_is_static() {
        let reg = ProviderRegistration {
            backend_id: "test-backend",
            factory: || panic!("not called"),
        };
        assert_eq!(reg.backend_id, "test-backend");
    }
}
