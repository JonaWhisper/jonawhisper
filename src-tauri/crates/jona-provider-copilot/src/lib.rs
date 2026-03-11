use jona_types::{
    CloudProvider, Provider, ProviderError, ProviderPreset, ProviderRegistration,
    TranscriptionResult,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::LazyLock;
use tokio::sync::Mutex;
use std::time::Instant;

static ASYNC_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Cached Copilot JWT: (full_api_key, jwt, fetched_at). GitHub Copilot JWTs
/// typically last 30 min; we use a 25 min TTL to refresh before expiry.
/// Keyed by the full OAuth token string so switching accounts invalidates the cache.
static JWT_CACHE: LazyLock<Mutex<Option<(String, String, Instant)>>> =
    LazyLock::new(|| Mutex::new(None));

const JWT_TTL_SECS: u64 = 25 * 60; // 25 minutes

/// GitHub Copilot backend — exchanges a GitHub OAuth token for a short-lived
/// Copilot JWT, then uses it with the OpenAI-compatible chat endpoint.
pub struct CopilotBackend;

/// Exchange a GitHub OAuth token (`gho_...`) for a Copilot JWT (network call).
async fn fetch_token(github_token: &str) -> Result<String, ProviderError> {
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

/// Get a cached JWT or exchange for a fresh one if expired/missing.
/// Holds the mutex lock across the full check-fetch-update cycle to prevent
/// concurrent cold/expired cache misses from triggering duplicate HTTP requests.
async fn exchange_token(github_token: &str) -> Result<String, ProviderError> {
    // Hold lock across the entire check-fetch-update cycle to prevent races.
    // tokio::sync::Mutex is Send-safe across .await points.
    let mut cache = JWT_CACHE.lock().await;

    if let Some((ref token_key, ref jwt, fetched_at)) = *cache {
        if token_key == github_token && fetched_at.elapsed().as_secs() < JWT_TTL_SECS {
            return Ok(jwt.clone());
        }
    }

    // Still holding lock — other callers wait during the HTTP call (~200ms).
    // Acceptable for a desktop app where Copilot LLM calls are sequential in practice.
    let jwt = fetch_token(github_token).await?;
    *cache = Some((github_token.to_string(), jwt.clone(), Instant::now()));

    Ok(jwt)
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
            let api_key = provider.api_key.trim();
            if api_key.is_empty() {
                return Err(ProviderError::NotConfigured(
                    "GitHub Copilot requires an OAuth token (gho_...)".into(),
                ));
            }

            let jwt = exchange_token(api_key).await?;

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
