use super::*;
use crate::state::ApiServerConfig;
use std::path::Path;
use std::sync::LazyLock;

static BLOCKING_CLIENT: LazyLock<reqwest::blocking::Client> =
    LazyLock::new(reqwest::blocking::Client::new);

pub struct OpenAIAPIEngine {
    servers: Vec<ApiServerConfig>,
}

impl OpenAIAPIEngine {
    pub fn new(servers: Vec<ApiServerConfig>) -> Self {
        Self { servers }
    }
}

impl ASREngine for OpenAIAPIEngine {
    fn engine_id(&self) -> &str { "openai-api" }
    fn display_name(&self) -> &str { "OpenAI API" }

    fn models(&self) -> Vec<ASRModel> {
        self.servers
            .iter()
            .map(|server| ASRModel {
                id: format!("openai-api:{}", server.id),
                engine_id: "openai-api".into(),
                label: server.name.clone(),
                filename: String::new(),
                url: server.url.clone(),
                size: String::new(),
                storage_dir: String::new(),
                download_type: DownloadType::RemoteAPI,
                download_marker: None,
            })
            .collect()
    }

    fn supported_languages(&self) -> Vec<Language> { common_languages() }

    fn install_hint(&self) -> &str { "" }

    fn resolve_executable(&self) -> Option<String> {
        // No local executable needed for API
        None
    }

    fn transcribe(&self, model: &ASRModel, audio_path: &Path, language: &str) -> Result<String, EngineError> {
        let server_id = model.id.strip_prefix("openai-api:").unwrap_or(&model.id);
        let server = self.servers.iter().find(|s| s.id == server_id)
            .ok_or_else(|| EngineError::ApiError("Server config not found".into()))?;

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
            .text("model", server.model.clone());

        if language != "auto" {
            form = form.text("language", language.to_string());
        }

        let url = format!("{}/v1/audio/transcriptions", server.url.trim_end_matches('/'));

        let mut req = client.post(&url).multipart(form);
        if !server.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", server.api_key));
        }

        let response = req.send().map_err(|e| EngineError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(EngineError::ApiError(format!("HTTP {}: {}", status, body)));
        }

        // Try JSON response first (OpenAI format), fall back to plain text
        let body = response.text().map_err(|e| EngineError::ApiError(e.to_string()))?;
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(text) = json.get("text").and_then(|t| t.as_str()) {
                return Ok(text.to_string());
            }
        }
        Ok(body)
    }
}
