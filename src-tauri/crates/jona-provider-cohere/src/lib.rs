use jona_types::{
    CloudProvider, FieldType, PresetField, Provider, ProviderError, ProviderPreset,
    ProviderRegistration, TranscriptionResult,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::LazyLock;

static ASYNC_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Cohere LLM — uses the v2 Chat API (non-OpenAI format).
pub struct CohereBackend;

impl CloudProvider for CohereBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        _model: &str,
        _audio_path: &Path,
        _language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        Err(ProviderError::NotConfigured(format!(
            "Provider '{}' does not support ASR transcription",
            provider.name
        )))
    }

    fn chat_completion<'a>(
        &'a self,
        provider: &'a Provider,
        model: &'a str,
        system: &'a str,
        user_message: &'a str,
        temperature: f32,
        max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            provider.validate_url().map_err(ProviderError::Http)?;

            let api_key = provider.api_key.trim();
            if api_key.is_empty() {
                return Err(ProviderError::NotConfigured(format!(
                    "Provider '{}' is missing an API key for Cohere",
                    provider.name
                )));
            }

            let request = CohereRequest {
                model: model.to_string(),
                messages: vec![
                    CohereMessage {
                        role: "system",
                        content: system.to_string(),
                    },
                    CohereMessage {
                        role: "user",
                        content: user_message.to_string(),
                    },
                ],
                temperature,
                max_tokens,
            };

            let url = format!("{}/v2/chat", provider.base_url());

            let response = ASYNC_CLIENT
                .post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&request)
                .send()
                .await
                .map_err(|e| ProviderError::Http(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_else(|e| {
                    log::warn!("Failed to decode Cohere error body: {e}");
                    String::new()
                });
                return Err(ProviderError::Api { status, body });
            }

            let chat_response: CohereResponse = response
                .json()
                .await
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

            // Cohere v2: message.content[0].text
            chat_response
                .message
                .content
                .into_iter()
                .next()
                .map(|c| c.text.trim().to_string())
                .ok_or_else(|| {
                    ProviderError::InvalidResponse("No content in response".into())
                })
        })
    }

    fn list_models<'a>(
        &'a self,
        _provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            Ok(vec![
                "command-r-plus".into(),
                "command-r".into(),
                "command-a-03-2025".into(),
            ])
        })
    }
}

#[derive(Serialize)]
struct CohereMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct CohereRequest {
    model: String,
    messages: Vec<CohereMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct CohereResponse {
    message: CohereMessageResponse,
}

#[derive(Deserialize)]
struct CohereMessageResponse {
    content: Vec<CohereContent>,
}

#[derive(Deserialize)]
struct CohereContent {
    text: String,
}

inventory::submit! { ProviderRegistration {
    backend_id: "cohere",
    factory: || Box::new(CohereBackend),
}}

inventory::submit! { ProviderPreset {
    id: "cohere", display_name: "Cohere",
    base_url: "https://api.cohere.com", backend_id: "cohere",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #39d353, #2ea043)",
    default_asr_models: &[],
    default_llm_models: &["command-r-plus", "command-r"],
    extra_fields: &[
        PresetField {
            id: "api_key",
            label: "API Key",
            field_type: FieldType::Password,
            required: true,
            placeholder: "",
            default_value: "",
            options: &[],
            sensitive: true,
        },
    ],
    hidden_fields: &[],
}}
