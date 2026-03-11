use jona_types::{
    CloudProvider, Provider, ProviderError, ProviderPreset, ProviderRegistration,
    TranscriptionResult,
};
use serde::Deserialize;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::LazyLock;

static BLOCKING_CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
});

/// Gladia ASR — async 3-step workflow: upload audio → create transcription → poll result_url.
pub struct GladiaBackend;

#[derive(Deserialize)]
struct UploadResponse {
    audio_url: String,
}

#[derive(Deserialize)]
struct InitResponse {
    result_url: String,
}

#[derive(Deserialize)]
struct PollResponse {
    status: String,
    #[serde(default)]
    result: Option<TranscriptionResult_>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct TranscriptionResult_ {
    #[serde(default)]
    transcription: Option<Transcription>,
}

#[derive(Deserialize)]
struct Transcription {
    #[serde(default)]
    full_transcript: Option<String>,
}

impl CloudProvider for GladiaBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        _model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        if provider.api_key.is_empty() {
            return Err(ProviderError::NotConfigured(
                "Gladia API key is not configured".into(),
            ));
        }
        provider.validate_url().map_err(ProviderError::Http)?;

        let base = provider.base_url();
        let api_key = provider.api_key.as_str();

        // Step 1: Upload audio file via multipart
        let file_bytes = std::fs::read(audio_path)?;
        let file_part = reqwest::blocking::multipart::Part::bytes(file_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let form = reqwest::blocking::multipart::Form::new().part("audio", file_part);

        let upload_resp = BLOCKING_CLIENT
            .post(format!("{}/v2/upload", base))
            .header("x-gladia-key", api_key)
            .multipart(form)
            .send()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !upload_resp.status().is_success() {
            let status = upload_resp.status().as_u16();
            let body = upload_resp.text().unwrap_or_default();
            return Err(ProviderError::Api { status, body });
        }

        let upload: UploadResponse = upload_resp
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        // Step 2: Initiate transcription with audio_url
        let mut body = serde_json::json!({
            "audio_url": upload.audio_url,
        });
        if language != "auto" {
            body["detect_language"] = serde_json::Value::Bool(false);
            body["language"] = serde_json::Value::String(language.to_string());
        } else {
            body["detect_language"] = serde_json::Value::Bool(true);
        }

        let init_resp = BLOCKING_CLIENT
            .post(format!("{}/v2/pre-recorded", base))
            .header("x-gladia-key", api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !init_resp.status().is_success() {
            let status = init_resp.status().as_u16();
            let body = init_resp.text().unwrap_or_default();
            return Err(ProviderError::Api { status, body });
        }

        let init: InitResponse = init_resp
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        // Step 3: Poll result_url until status is "done" or "error"
        let max_polls = 60; // 60 * 2s = 2 minutes max
        for _ in 0..max_polls {
            std::thread::sleep(std::time::Duration::from_secs(2));

            let poll_resp = BLOCKING_CLIENT
                .get(&init.result_url)
                .header("x-gladia-key", api_key)
                .send()
                .map_err(|e| ProviderError::Http(e.to_string()))?;

            if !poll_resp.status().is_success() {
                let status = poll_resp.status().as_u16();
                let body = poll_resp.text().unwrap_or_default();
                return Err(ProviderError::Api { status, body });
            }

            let result: PollResponse = poll_resp
                .json()
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

            match result.status.as_str() {
                "done" => {
                    let text = result
                        .result
                        .and_then(|r| r.transcription)
                        .and_then(|t| t.full_transcript)
                        .unwrap_or_default();
                    return Ok(TranscriptionResult::text_only(text));
                }
                "error" => {
                    let msg = result.error.unwrap_or_else(|| "Unknown error".into());
                    return Err(ProviderError::InvalidResponse(msg));
                }
                _ => continue, // "queued" or "processing"
            }
        }

        Err(ProviderError::InvalidResponse(
            "Gladia transcription timed out after polling".into(),
        ))
    }

    fn chat_completion<'a>(
        &'a self,
        provider: &'a Provider,
        _model: &'a str,
        _system: &'a str,
        _user_message: &'a str,
        _temperature: f32,
        _max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            Err(ProviderError::NotConfigured(format!(
                "Provider '{}' does not support LLM chat",
                provider.name
            )))
        })
    }

    fn list_models<'a>(
        &'a self,
        _provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move { Ok(vec!["whisper-zero".into()]) })
    }
}

inventory::submit! { ProviderRegistration {
    backend_id: "gladia",
    factory: || Box::new(GladiaBackend),
}}

inventory::submit! { ProviderPreset {
    id: "gladia", display_name: "Gladia",
    base_url: "https://api.gladia.io", backend_id: "gladia",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #a855f7, #7c3aed)",
    default_asr_models: &["whisper-zero"],
    default_llm_models: &[],
    extra_fields: &[],
    hidden_fields: &[],
}}
