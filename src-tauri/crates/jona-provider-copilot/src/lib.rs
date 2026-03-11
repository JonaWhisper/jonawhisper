use jona_types::{
    CloudProvider, Provider, ProviderError, ProviderPreset, ProviderRegistration,
    TranscriptionResult,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

static ASYNC_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Cached Copilot JWT: (oauth_token_hash, jwt, fetched_at). GitHub Copilot JWTs
/// typically last 30 min; we use a 25 min TTL to refresh before expiry.
/// Keyed by a hash of the OAuth token so switching accounts invalidates the cache.
static JWT_CACHE: LazyLock<Mutex<Option<(u64, String, Instant)>>> =
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

/// Simple hash of the OAuth token for cache keying (not cryptographic, just identity).
fn hash_token(token: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut hasher);
    hasher.finish()
}

/// Get a cached JWT or exchange for a fresh one if expired/missing.
/// Cache is keyed by the OAuth token hash so switching accounts invalidates it.
async fn exchange_token(github_token: &str) -> Result<String, ProviderError> {
    let token_hash = hash_token(github_token);

    // Check cache — must match the current OAuth token.
    // unwrap_or_else recovers from a poisoned mutex (prior panic left stale data).
    {
        let guard = JWT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cached_hash, ref jwt, fetched_at)) = *guard {
            if cached_hash == token_hash && fetched_at.elapsed().as_secs() < JWT_TTL_SECS {
                return Ok(jwt.clone());
            }
        }
    }

    // Cache miss, expired, or different token — fetch a new JWT
    let jwt = fetch_token(github_token).await?;

    {
        let mut guard = JWT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some((token_hash, jwt.clone(), Instant::now()));
    }

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
}}
