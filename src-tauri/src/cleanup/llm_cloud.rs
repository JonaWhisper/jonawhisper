use jona_engines::llm_prompt::{sanitize_output, system_prompt, LlmError};
use crate::state::Provider;

/// Clean up transcribed text using a cloud LLM.
pub async fn cleanup_text(text: &str, language: &str, provider: &Provider, model: &str, max_tokens: u32) -> Result<String, LlmError> {
    if provider.url.is_empty() || model.is_empty() {
        return Err(LlmError::NotConfigured);
    }
    provider.validate_url().map_err(LlmError::Http)?;

    let system = system_prompt(language);
    let raw = jona_provider::backend(provider.kind)
        .chat_completion(provider, model, &system, text, 0.1, max_tokens)
        .await
        .map_err(|e| match e {
            jona_provider::ProviderError::Http(msg) => LlmError::Http(msg),
            jona_provider::ProviderError::Api { status, body } => LlmError::Api { status, body },
            jona_provider::ProviderError::InvalidResponse(msg) => LlmError::InvalidResponse(msg),
            other => LlmError::Http(other.to_string()),
        })?;

    sanitize_output(&raw, text.len())
}
