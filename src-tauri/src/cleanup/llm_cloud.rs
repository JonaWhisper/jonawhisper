use super::llm_prompt::{sanitize_output, system_prompt, LlmError};
use crate::state::Provider;
use serde::{Deserialize, Serialize};

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

/// Send an HTTP request and check for errors. Returns the successful response.
async fn send_and_check(req: reqwest::RequestBuilder) -> Result<reqwest::Response, LlmError> {
    let response = req.send().await.map_err(|e| LlmError::Http(e.to_string()))?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(LlmError::Api { status, body });
    }
    Ok(response)
}

/// Clean up transcribed text using a cloud LLM.
pub async fn cleanup_text(text: &str, language: &str, provider: &Provider, model: &str, max_tokens: u32) -> Result<String, LlmError> {
    if provider.url.is_empty() || model.is_empty() {
        return Err(LlmError::NotConfigured);
    }
    provider.validate_url().map_err(LlmError::Http)?;

    let raw = if provider.kind.is_anthropic_format() {
        call_anthropic(text, language, provider, model, max_tokens).await?
    } else {
        call_openai_compatible(text, language, provider, model, max_tokens).await?
    };

    sanitize_output(&raw, text.len())
}

async fn call_openai_compatible(text: &str, language: &str, provider: &Provider, model: &str, max_tokens: u32) -> Result<String, LlmError> {
    let url = format!("{}/chat/completions", provider.base_url());

    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage { role: "system", content: system_prompt(language) },
            ChatMessage { role: "user", content: text.to_string() },
        ],
        temperature: 0.1,
        max_tokens,
    };

    let mut req = crate::http::CLIENT.post(&url).json(&request);
    if !provider.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", provider.api_key));
    }

    let response = send_and_check(req).await?;

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

async fn call_anthropic(text: &str, language: &str, provider: &Provider, model: &str, max_tokens: u32) -> Result<String, LlmError> {
    let url = format!("{}/messages", provider.base_url());

    let request = AnthropicRequest {
        model: model.to_string(),
        max_tokens,
        system: system_prompt(language),
        messages: vec![AnthropicMessage {
            role: "user",
            content: text.to_string(),
        }],
    };

    let mut req = crate::http::CLIENT
        .post(&url)
        .header("anthropic-version", "2023-06-01")
        .json(&request);
    if !provider.api_key.is_empty() {
        req = req.header("x-api-key", &provider.api_key);
    }

    let response = send_and_check(req).await?;

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
