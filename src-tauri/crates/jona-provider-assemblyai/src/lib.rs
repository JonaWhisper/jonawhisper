use jona_types::{
    CloudProvider, FieldType, PresetField, Provider, ProviderError, ProviderPreset,
    ProviderRegistration, TranscriptionResult,
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
        .unwrap_or_else(|e| {
            log::warn!("AssemblyAI: failed to build HTTP client with timeout: {e}, falling back to default");
            reqwest::blocking::Client::new()
        })
});

/// AssemblyAI ASR — asynchronous 3-step workflow: upload → create transcript → poll.
pub struct AssemblyAiBackend;

#[derive(Deserialize)]
struct UploadResponse {
    upload_url: String,
}

#[derive(Deserialize)]
struct TranscriptResponse {
    id: String,
    status: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

impl CloudProvider for AssemblyAiBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        provider.validate_url().map_err(ProviderError::Http)?;

        let api_key = provider.api_key.trim();
        if api_key.is_empty() {
            return Err(ProviderError::NotConfigured(format!(
                "Provider '{}' is missing an API key for AssemblyAI",
                provider.name
            )));
        }

        let base = provider.base_url();
        let auth_header = ("authorization", api_key);

        // Step 1: Upload audio file
        let file_bytes = std::fs::read(audio_path)?;
        let upload_resp = BLOCKING_CLIENT
            .post(format!("{}/v2/upload", base))
            .header(auth_header.0, auth_header.1)
            .header("Content-Type", "application/octet-stream")
            .body(file_bytes)
            .send()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !upload_resp.status().is_success() {
            let status = upload_resp.status().as_u16();
            let body = upload_resp.text().unwrap_or_else(|e| {
                log::warn!("AssemblyAI: failed to read upload error body: {e}");
                String::new()
            });
            return Err(ProviderError::Api { status, body });
        }

        let upload: UploadResponse = upload_resp
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        // Step 2: Create transcript
        let mut body = serde_json::json!({
            "audio_url": upload.upload_url,
        });
        if !model.is_empty() && model != "best" {
            body["speech_model"] = serde_json::Value::String(model.to_string());
        }
        if language != "auto" {
            body["language_code"] = serde_json::Value::String(language.to_string());
        }

        let create_resp = BLOCKING_CLIENT
            .post(format!("{}/v2/transcript", base))
            .header(auth_header.0, auth_header.1)
            .json(&body)
            .send()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !create_resp.status().is_success() {
            let status = create_resp.status().as_u16();
            let body = create_resp.text().unwrap_or_else(|e| {
                log::warn!("AssemblyAI: failed to read create-transcript error body: {e}");
                String::new()
            });
            return Err(ProviderError::Api { status, body });
        }

        let transcript: TranscriptResponse = create_resp
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        // Step 3: Poll until completed or error.
        // Capped at 90s to stay safely under the HTTP client's 120s timeout — if the
        // transcript isn't ready by then, we'd rather return a clear timeout error than
        // let the client silently drop the connection.
        // Stepped backoff: 1s (polls 0-4), 2s (5-14), 3s (15+).
        let poll_url = format!("{}/v2/transcript/{}", base, transcript.id);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
        let mut poll_count = 0u32;
        loop {
            let delay = if poll_count < 5 {
                1
            } else if poll_count < 15 {
                2
            } else {
                3
            };
            std::thread::sleep(std::time::Duration::from_secs(delay));
            poll_count += 1;
            if std::time::Instant::now() >= deadline {
                break;
            }

            // Per-request timeout capped to remaining time so the 90s deadline is enforced
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let poll_timeout = remaining.min(std::time::Duration::from_secs(15));

            let poll_resp = BLOCKING_CLIENT
                .get(&poll_url)
                .header(auth_header.0, auth_header.1)
                .timeout(poll_timeout)
                .send()
                .map_err(|e| ProviderError::Http(e.to_string()))?;

            if !poll_resp.status().is_success() {
                let status = poll_resp.status().as_u16();
                let body = poll_resp.text().unwrap_or_else(|e| {
                    log::warn!("AssemblyAI: failed to read poll error body: {e}");
                    String::new()
                });
                return Err(ProviderError::Api { status, body });
            }

            let result: TranscriptResponse = poll_resp
                .json()
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

            match result.status.as_str() {
                "completed" => {
                    let text = result.text.unwrap_or_default();
                    if text.is_empty() {
                        return Err(ProviderError::InvalidResponse(
                            "AssemblyAI returned an empty transcript".into(),
                        ));
                    }
                    return Ok(TranscriptionResult::text_only(text));
                }
                "error" => {
                    let msg = result.error.unwrap_or_else(|| "Unknown error".into());
                    return Err(ProviderError::InvalidResponse(msg));
                }
                "queued" | "processing" => {} // expected intermediate states
                unknown => {
                    return Err(ProviderError::InvalidResponse(format!(
                        "AssemblyAI returned unexpected transcript status: '{unknown}'"
                    )));
                }
            }
        }

        Err(ProviderError::InvalidResponse(
            "AssemblyAI transcript timed out after polling".into(),
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
        Box::pin(async move {
            Ok(vec![
                "best".into(),
                "nano".into(),
                "conformer-2".into(),
            ])
        })
    }
}

inventory::submit! { ProviderRegistration {
    backend_id: "assemblyai",
    factory: || Box::new(AssemblyAiBackend),
}}

inventory::submit! { ProviderPreset {
    id: "assemblyai", display_name: "AssemblyAI",
    base_url: "https://api.assemblyai.com", backend_id: "assemblyai",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #6366f1, #4f46e5)",
    default_asr_models: &["best", "nano"],
    default_llm_models: &[],
    extra_fields: &[
        PresetField {
            id: "api_key",
            label: "API Key",
            field_type: FieldType::Password,
            required: true,
            placeholder: "",
            default_value: "",
            options: &[],
            sensitive: true,
        },
    ],
    hidden_fields: &[],
}}
