use crate::state::Provider;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("LLM not configured")]
    NotConfigured,
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error: {status} {body}")]
    Api { status: u16, body: String },
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
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

// Anthropic Messages API types
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

fn system_prompt(language: &str) -> String {
    let lang_name = match language {
        "fr" => "French",
        "en" => "English",
        "es" => "Spanish",
        "de" => "German",
        _ => "the same language as the input",
    };

    format!(
        "You are a dictation text cleaner. Your job is to clean up raw speech-to-text output.\n\
         Rules:\n\
         - Fix punctuation, capitalization, and spacing\n\
         - Remove filler words and speech artifacts (um, uh, etc.)\n\
         - Do NOT change the meaning or rephrase\n\
         - Do NOT add information that wasn't in the original\n\
         - Output language: {lang_name}\n\
         - Reply with ONLY the cleaned text, nothing else"
    )
}

/// Clean up transcribed text using an LLM.
/// Returns the cleaned text, or an error.
pub async fn cleanup_text(text: &str, language: &str, provider: &Provider, model: &str) -> Result<String, LlmError> {
    if provider.url.is_empty() || model.is_empty() {
        return Err(LlmError::NotConfigured);
    }

    if provider.kind.is_anthropic_format() {
        call_anthropic(text, language, provider, model).await
    } else {
        call_openai_compatible(text, language, provider, model).await
    }
}

async fn call_openai_compatible(text: &str, language: &str, provider: &Provider, model: &str) -> Result<String, LlmError> {
    let url = format!("{}/v1/chat/completions", provider.url.trim_end_matches('/'));

    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage { role: "system", content: system_prompt(language) },
            ChatMessage { role: "user", content: text.to_string() },
        ],
        temperature: 0.1,
        max_tokens: 4096,
    };

    let mut req = HTTP_CLIENT.post(&url).json(&request);
    if !provider.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", provider.api_key));
    }

    let response = req.send().await.map_err(|e| LlmError::Http(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(LlmError::Api { status, body });
    }

    let chat_response: ChatResponse = response
        .json()
        .await
        .map_err(|e| LlmError::InvalidResponse(e.to_string()))?;

    chat_response
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content.trim().to_string())
        .ok_or_else(|| LlmError::InvalidResponse("No choices in response".into()))
}

async fn call_anthropic(text: &str, language: &str, provider: &Provider, model: &str) -> Result<String, LlmError> {
    let url = format!("{}/v1/messages", provider.url.trim_end_matches('/'));

    let request = AnthropicRequest {
        model: model.to_string(),
        max_tokens: 4096,
        system: system_prompt(language),
        messages: vec![AnthropicMessage {
            role: "user",
            content: text.to_string(),
        }],
    };

    let mut req = HTTP_CLIENT
        .post(&url)
        .header("anthropic-version", "2023-06-01")
        .json(&request);
    if !provider.api_key.is_empty() {
        req = req.header("x-api-key", &provider.api_key);
    }

    let response = req.send().await.map_err(|e| LlmError::Http(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(LlmError::Api { status, body });
    }

    let anthropic_response: AnthropicResponse = response
        .json()
        .await
        .map_err(|e| LlmError::InvalidResponse(e.to_string()))?;

    anthropic_response
        .content
        .into_iter()
        .next()
        .map(|c| c.text.trim().to_string())
        .ok_or_else(|| LlmError::InvalidResponse("No content in response".into()))
}
