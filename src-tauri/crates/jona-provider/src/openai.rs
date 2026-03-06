use crate::{CloudProvider, ProviderError};
use jona_types::Provider;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::LazyLock;

static BLOCKING_CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
});

static ASYNC_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

pub struct OpenAICompatibleBackend;

impl CloudProvider for OpenAICompatibleBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<String, ProviderError> {
        provider.validate_url().map_err(ProviderError::Http)?;

        let file_bytes = std::fs::read(audio_path)?;
        let file_name = audio_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let file_part = reqwest::blocking::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let mut form = reqwest::blocking::multipart::Form::new()
            .part("file", file_part)
            .text("model", model.to_string());

        if language != "auto" {
            form = form.text("language", language.to_string());
        }

        let url = format!("{}/audio/transcriptions", provider.base_url());

        let mut req = BLOCKING_CLIENT.post(&url).multipart(form);
        if !provider.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", provider.api_key));
        }

        let response = req.send().map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(ProviderError::Api { status, body });
        }

        let body = response.text().map_err(|e| ProviderError::Http(e.to_string()))?;
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(text) = json.get("text").and_then(|t| t.as_str()) {
                return Ok(text.to_string());
            }
        }
        Ok(body)
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
            let url = format!("{}/chat/completions", provider.base_url());

            let request = ChatRequest {
                model: model.to_string(),
                messages: vec![
                    ChatMessage { role: "system", content: system.to_string() },
                    ChatMessage { role: "user", content: user_message.to_string() },
                ],
                temperature,
                max_tokens,
            };

            let mut req = ASYNC_CLIENT.post(&url).json(&request);
            if !provider.api_key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", provider.api_key));
            }

            let response = send_and_check(req).await?;

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
        provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let url = format!("{}/models", provider.base_url());

            let mut req = ASYNC_CLIENT.get(&url);
            if !provider.api_key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", provider.api_key));
            }

            let response = send_and_check(req).await?;
            parse_model_ids(response).await
        })
    }
}

// -- Request/Response types --

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

// -- Shared helpers --

async fn send_and_check(req: reqwest::RequestBuilder) -> Result<reqwest::Response, ProviderError> {
    let response = req.send().await.map_err(|e| ProviderError::Http(e.to_string()))?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(ProviderError::Api { status, body });
    }
    Ok(response)
}

/// Parse model IDs from OpenAI-compatible response ({data:[...]} or bare [...]).
pub(crate) async fn parse_model_ids(response: reqwest::Response) -> Result<Vec<String>, ProviderError> {
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

    let models_array = json.get("data").and_then(|d| d.as_array())
        .or_else(|| json.as_array());

    let ids: Vec<String> = models_array
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if ids.is_empty() {
        return Err(ProviderError::InvalidResponse("No models found in response".into()));
    }

    Ok(ids)
}
