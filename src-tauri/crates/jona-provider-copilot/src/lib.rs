use jona_types::{
    CloudProvider, Provider, ProviderError, ProviderPreset, ProviderRegistration,
    TranscriptionResult,
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

/// GitHub Copilot backend — exchanges a GitHub OAuth token for a short-lived
/// Copilot JWT, then uses it with the OpenAI-compatible chat endpoint.
pub struct CopilotBackend;

/// Exchange a GitHub OAuth token (`gho_...`) for a Copilot JWT.
async fn exchange_token(github_token: &str) -> Result<String, ProviderError> {
    let response = ASYNC_CLIENT
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Authorization", format!("token {}", github_token))
        .header("User-Agent", "JonaWhisper/1.0")
        .send()
        .await
        .map_err(|e| ProviderError::Http(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(ProviderError::Api { status, body });
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

    json.get("token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ProviderError::InvalidResponse("No token in response".into()))
}

impl CloudProvider for CopilotBackend {
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
            let jwt = exchange_token(&provider.api_key).await?;

            let request = ChatRequest {
                model: model.to_string(),
                messages: vec![
                    ChatMessage { role: "system", content: system.to_string() },
                    ChatMessage { role: "user", content: user_message.to_string() },
                ],
                temperature,
                max_tokens,
            };

            let response = ASYNC_CLIENT
                .post("https://api.githubcopilot.com/chat/completions")
                .header("Authorization", format!("Bearer {}", jwt))
                .header("Editor-Version", "JonaWhisper/1.0")
                .header("Copilot-Integration-Id", "jonawhisper")
                .header("Openai-Organization", "github-copilot")
                .json(&request)
                .send()
                .await
                .map_err(|e| ProviderError::Http(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(ProviderError::Api { status, body });
            }

            let chat_response: ChatResponse = response
                .json()
                .await
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

            chat_response
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content.trim().to_string())
                .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".into()))
        })
    }

    fn list_models<'a>(
        &'a self,
        _provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            // Copilot doesn't expose a model list — return known models
            Ok(vec![
                "gpt-4o".into(),
                "gpt-4o-mini".into(),
                "claude-3.5-sonnet".into(),
            ])
        })
    }
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

inventory::submit! { ProviderRegistration {
    backend_id: "copilot",
    factory: || Box::new(CopilotBackend),
}}

inventory::submit! { ProviderPreset {
    id: "copilot", display_name: "GitHub Copilot",
    base_url: "https://api.githubcopilot.com", backend_id: "copilot",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #24292e, #586069)",
    default_asr_models: &[],
    default_llm_models: &["gpt-4o", "gpt-4o-mini"],
    extra_fields: &[],
    hidden_fields: &["base_url"],
}}
