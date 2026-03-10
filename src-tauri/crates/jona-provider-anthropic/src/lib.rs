use jona_types::{
    parse_model_ids_from_json, ApiFormat, CloudProvider, Provider, ProviderError,
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
        _temperature: f32,
        max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let url = format!("{}/messages", provider.base_url());

            let request = AnthropicRequest {
                model: model.to_string(),
                max_tokens,
                system: system.to_string(),
                messages: vec![AnthropicMessage {
                    role: "user",
                    content: user_message.to_string(),
                }],
            };

            let mut req = ASYNC_CLIENT
                .post(&url)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .json(&request);
            if !provider.api_key.is_empty() {
                req = req.header("x-api-key", &provider.api_key);
            }

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

            let mut req = ASYNC_CLIENT
                .get(&url)
                .header("anthropic-version", ANTHROPIC_VERSION);
            if !provider.api_key.is_empty() {
                req = req.header("x-api-key", &provider.api_key);
            }

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
        let body = response.text().await.unwrap_or_default();
        return Err(ProviderError::Api { status, body });
    }
    Ok(response)
}

inventory::submit! { ProviderRegistration {
    api_format: ApiFormat::Anthropic,
    factory: || Box::new(AnthropicBackend),
}}

inventory::submit! { ProviderPreset {
    id: "anthropic", display_name: "Anthropic",
    base_url: "https://api.anthropic.com/v1", api_format: ApiFormat::Anthropic,
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #d97706, #b45309)",
    default_asr_models: &[],
    default_llm_models: &["claude-haiku-4-5-20251001", "claude-sonnet-4-5-20250514", "claude-opus-4-6-20250626"],
}}
