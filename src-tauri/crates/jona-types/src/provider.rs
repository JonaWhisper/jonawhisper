//! Cloud provider types, trait, and auto-registration.

use crate::{ApiFormat, Provider, TranscriptionResult};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

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

/// Auto-registration entry for cloud provider backends (one per ApiFormat).
pub struct ProviderRegistration {
    pub api_format: ApiFormat,
    pub factory: fn() -> Box<dyn CloudProvider>,
}

inventory::collect!(ProviderRegistration);

/// Data-driven preset for a known cloud provider.
/// Registered via `inventory::submit!` in provider crates.
pub struct ProviderPreset {
    pub id: &'static str,
    pub display_name: &'static str,
    pub base_url: &'static str,
    pub api_format: ApiFormat,
    pub supports_asr: bool,
    pub supports_llm: bool,
    pub gradient: &'static str,
    pub default_asr_models: &'static [&'static str],
    pub default_llm_models: &'static [&'static str],
}

inventory::collect!(ProviderPreset);

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
