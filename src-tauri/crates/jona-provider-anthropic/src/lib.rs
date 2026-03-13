use jona_types::{
    parse_model_ids_from_json, CloudProvider, FieldType, PresetField, Provider, ProviderError,
    ProviderPreset, ProviderRegistration, TranscriptionResult,
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

const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Add the correct auth header and beta flags.
/// OAuth tokens (sk-ant-oat*) use Bearer + oauth beta header.
/// Standard API keys use x-api-key.
fn add_auth(req: reqwest::RequestBuilder, api_key: &str) -> reqwest::RequestBuilder {
    if api_key.is_empty() {
        return req;
    }
    if api_key.starts_with("sk-ant-oat") {
        req.header("Authorization", format!("Bearer {}", api_key))
            .header("anthropic-beta", "oauth-2025-04-20")
    } else {
        req.header("x-api-key", api_key)
    }
}

pub struct AnthropicBackend;

impl CloudProvider for AnthropicBackend {
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

            let url = format!("{}/messages", provider.base_url());

            let request = AnthropicRequest {
                model: model.to_string(),
                max_tokens,
                temperature,
                system: system.to_string(),
                messages: vec![AnthropicMessage {
                    role: "user",
                    content: user_message.to_string(),
                }],
            };

            let api_key = provider.api_key.trim();
            let req = ASYNC_CLIENT
                .post(&url)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .json(&request);
            let req = add_auth(req, api_key);

            let response = send_and_check(req).await?;

            let anthropic_response: AnthropicResponse = response
                .json()
                .await
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

            anthropic_response
                .content
                .into_iter()
                .next()
                .map(|c| c.text.trim().to_string())
                .ok_or_else(|| ProviderError::InvalidResponse("No content in response".into()))
        })
    }

    fn list_models<'a>(
        &'a self,
        provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let url = format!("{}/models", provider.base_url());

            let api_key = provider.api_key.trim();
            let req = ASYNC_CLIENT
                .get(&url)
                .header("anthropic-version", ANTHROPIC_VERSION);
            let req = add_auth(req, api_key);

            let response = send_and_check(req).await?;
            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;
            parse_model_ids_from_json(&json)
        })
    }
}

// -- Request/Response types --

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    temperature: f32,
    system: String,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

async fn send_and_check(req: reqwest::RequestBuilder) -> Result<reqwest::Response, ProviderError> {
    let response = req
        .send()
        .await
        .map_err(|e| ProviderError::Http(e.to_string()))?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_else(|e| {
            log::warn!("Anthropic: failed to decode error body: {e}");
            String::new()
        });
        return Err(ProviderError::Api { status, body });
    }
    Ok(response)
}

inventory::submit! { ProviderRegistration {
    backend_id: "anthropic",
    factory: || Box::new(AnthropicBackend),
}}

inventory::submit! { ProviderPreset {
    id: "anthropic", display_name: "Anthropic",
    base_url: "https://api.anthropic.com/v1", backend_id: "anthropic",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #d97706, #b45309)",
    default_asr_models: &[],
    default_llm_models: &["claude-haiku-4-5-20251001", "claude-sonnet-4-5-20250514", "claude-opus-4-6-20250626"],
    extra_fields: &[
        PresetField {
            id: "api_key",
            label: "API Key",
            field_type: FieldType::Password,
            required: true,
            placeholder: "sk-ant-...",
            default_value: "",
            options: &[],
            sensitive: true,
        },
    ],
    hidden_fields: &[],
}}
