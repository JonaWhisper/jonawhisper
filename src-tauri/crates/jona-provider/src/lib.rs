mod openai;
mod anthropic;

use jona_types::{Provider, ProviderKind, TranscriptionResult};
use std::path::Path;
use std::pin::Pin;
use std::future::Future;

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

/// Get the appropriate cloud provider backend for a given kind.
pub fn backend(kind: ProviderKind) -> &'static dyn CloudProvider {
    match kind {
        ProviderKind::Anthropic => &anthropic::AnthropicBackend,
        _ => &openai::OpenAICompatibleBackend,
    }
}
