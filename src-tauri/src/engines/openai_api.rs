use super::EngineError;
use crate::state::Provider;
use std::path::Path;
use std::sync::LazyLock;

static BLOCKING_CLIENT: LazyLock<reqwest::blocking::Client> =
    LazyLock::new(reqwest::blocking::Client::new);

pub fn transcribe(
    provider: &Provider,
    model: &str,
    audio_path: &Path,
    language: &str,
) -> Result<String, EngineError> {
    let client = &*BLOCKING_CLIENT;

    let file_bytes = std::fs::read(audio_path)
        .map_err(|e| EngineError::LaunchFailed(e.to_string()))?;

    let file_name = audio_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let file_part = reqwest::blocking::multipart::Part::bytes(file_bytes)
        .file_name(file_name)
        .mime_str("audio/wav")
        .map_err(|e| EngineError::ApiError(e.to_string()))?;

    let mut form = reqwest::blocking::multipart::Form::new()
        .part("file", file_part)
        .text("model", model.to_string());

    if language != "auto" {
        form = form.text("language", language.to_string());
    }

    let url = format!("{}/v1/audio/transcriptions", provider.url.trim_end_matches('/'));

    let mut req = client.post(&url).multipart(form);
    if !provider.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", provider.api_key));
    }

    let response = req.send().map_err(|e| EngineError::ApiError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(EngineError::ApiError(format!("HTTP {}: {}", status, body)));
    }

    let body = response.text().map_err(|e| EngineError::ApiError(e.to_string()))?;
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(text) = json.get("text").and_then(|t| t.as_str()) {
            return Ok(text.to_string());
        }
    }
    Ok(body)
}
