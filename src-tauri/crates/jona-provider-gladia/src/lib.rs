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

/// Stepped polling backoff: 1s for first 5 polls, 2s up to 15 polls, then 3s.
fn poll_delay(poll_index: u32) -> std::time::Duration {
    let secs = if poll_index < 5 {
        1
    } else if poll_index < 15 {
        2
    } else {
        3
    };
    std::time::Duration::from_secs(secs)
}

impl CloudProvider for GladiaBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        _model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        if provider.api_key.trim().is_empty() {
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
        // Stepped backoff: 1s (polls 0-4), 2s (5-14), 3s (15+). ~120s total budget.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let mut poll_count = 0u32;
        loop {
            std::thread::sleep(poll_delay(poll_count));
            poll_count += 1;
            if std::time::Instant::now() >= deadline {
                break;
            }

            // Per-request timeout capped to remaining time so the deadline is enforced
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let poll_timeout = remaining.min(std::time::Duration::from_secs(15));

            let poll_resp = BLOCKING_CLIENT
                .get(&init.result_url)
                .header("x-gladia-key", api_key)
                .timeout(poll_timeout)
                .send()
                .map_err(|e| ProviderError::Http(e.to_string()))?;

            if !poll_resp.status().is_success() {
                let status = poll_resp.status().as_u16();
                let body = poll_resp.text().unwrap_or_default();
                return Err(ProviderError::Api { status, body });
            }

            // Parse as raw JSON and use pointer for robust field access
            let json: serde_json::Value = poll_resp
                .json()
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

            let status = json
                .get("status")
                .and_then(|v| v.as_str());

            match status {
                Some("done") => {
                    let text = json
                        .pointer("/result/transcription/full_transcript")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ProviderError::InvalidResponse(
                            "Gladia response missing /result/transcription/full_transcript".into(),
                        ))?;
                    return Ok(TranscriptionResult::text_only(text.to_string()));
                }
                Some("error") => {
                    let msg = json
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error");
                    return Err(ProviderError::InvalidResponse(msg.to_string()));
                }
                Some("queued" | "processing") => continue,
                Some(other) => {
                    return Err(ProviderError::InvalidResponse(format!(
                        "Gladia returned unexpected status: '{other}'"
                    )));
                }
                None => {
                    return Err(ProviderError::InvalidResponse(
                        "Gladia response missing 'status' field".into(),
                    ));
                }
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
}}
