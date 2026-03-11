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
            log::warn!("Speechmatics: failed to build HTTP client with timeout: {e}, falling back to default");
            reqwest::blocking::Client::new()
        })
});

/// Speechmatics Batch ASR — 3-step workflow: create job (multipart) → poll for completion → fetch transcript.
pub struct SpeechmaticsBackend;

#[derive(Deserialize)]
struct CreateJobResponse {
    id: String,
}

#[derive(Deserialize)]
struct JobStatusResponse {
    job: JobInfo,
}

#[derive(Deserialize)]
struct JobInfo {
    status: String,
}

#[derive(Deserialize)]
struct TranscriptResponse {
    #[serde(default)]
    results: Vec<TranscriptItem>,
}

#[derive(Deserialize)]
struct TranscriptItem {
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    content: String,
}

impl CloudProvider for SpeechmaticsBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        let api_key = provider.api_key.trim();
        if api_key.is_empty() {
            return Err(ProviderError::NotConfigured(
                "Speechmatics API key is not configured".into(),
            ));
        }
        provider.validate_url().map_err(ProviderError::Http)?;

        let base = provider.base_url();
        let bearer = format!("Bearer {api_key}");

        // Build transcription config JSON
        let operating_point = if model.is_empty() || model == "enhanced" {
            "enhanced"
        } else {
            model
        };
        let lang = if language == "auto" { "auto" } else { language };

        let config = serde_json::json!({
            "type": "transcription",
            "transcription_config": {
                "language": lang,
                "operating_point": operating_point,
            }
        });

        // Step 1: Create job with multipart (data_file + config)
        let file_bytes = std::fs::read(audio_path)?;
        let file_part = reqwest::blocking::multipart::Part::bytes(file_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let config_part = reqwest::blocking::multipart::Part::text(config.to_string())
            .mime_str("application/json")
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let form = reqwest::blocking::multipart::Form::new()
            .part("data_file", file_part)
            .part("config", config_part);

        let create_resp = BLOCKING_CLIENT
            .post(format!("{}/v2/jobs", base))
            .header("Authorization", &bearer)
            .multipart(form)
            .send()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !create_resp.status().is_success() {
            let status = create_resp.status().as_u16();
            let body = create_resp.text().unwrap_or_else(|e| {
                log::warn!("Speechmatics: failed to read create-job error body: {e}");
                String::new()
            });
            return Err(ProviderError::Api { status, body });
        }

        let job: CreateJobResponse = create_resp
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        // Step 2: Poll job status until "done" or "rejected"
        // Stepped backoff: 1s (polls 0-4), 2s (5-14), 3s (15+). ~120s total budget.
        let status_url = format!("{}/v2/jobs/{}", base, job.id);
        let transcript_url = format!("{}/v2/jobs/{}/transcript?format=json-v2", base, job.id);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
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

            let poll_resp = BLOCKING_CLIENT
                .get(&status_url)
                .header("Authorization", &bearer)
                .send()
                .map_err(|e| ProviderError::Http(e.to_string()))?;

            if !poll_resp.status().is_success() {
                let status = poll_resp.status().as_u16();
                let body = poll_resp.text().unwrap_or_else(|e| {
                    log::warn!("Speechmatics: failed to read poll error body: {e}");
                    String::new()
                });
                return Err(ProviderError::Api { status, body });
            }

            let status_resp: JobStatusResponse = poll_resp
                .json()
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

            match status_resp.job.status.as_str() {
                "done" => {
                    // Fetch transcript
                    let tr_resp = BLOCKING_CLIENT
                        .get(&transcript_url)
                        .header("Authorization", &bearer)
                        .send()
                        .map_err(|e| ProviderError::Http(e.to_string()))?;

                    if !tr_resp.status().is_success() {
                        let status = tr_resp.status().as_u16();
                        let body = tr_resp.text().unwrap_or_else(|e| {
                            log::warn!("Speechmatics: failed to read transcript error body: {e}");
                            String::new()
                        });
                        return Err(ProviderError::Api { status, body });
                    }

                    let transcript: TranscriptResponse = tr_resp
                        .json()
                        .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

                    // Concatenate words and punctuation from results
                    let mut text = String::new();
                    for item in &transcript.results {
                        match item.item_type.as_str() {
                            "word" => {
                                if !text.is_empty() {
                                    text.push(' ');
                                }
                                text.push_str(&item.content);
                            }
                            "punctuation" => {
                                text.push_str(&item.content);
                            }
                            _ => {}
                        }
                    }

                    return Ok(TranscriptionResult::text_only(text));
                }
                "rejected" => {
                    return Err(ProviderError::InvalidResponse(
                        "Speechmatics job was rejected".into(),
                    ));
                }
                _ => continue, // "running", "waiting"
            }
        }

        Err(ProviderError::InvalidResponse(
            "Speechmatics job timed out after polling".into(),
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
            Ok(vec!["enhanced".into(), "standard".into()])
        })
    }
}

inventory::submit! { ProviderRegistration {
    backend_id: "speechmatics",
    factory: || Box::new(SpeechmaticsBackend),
}}

inventory::submit! { ProviderPreset {
    id: "speechmatics", display_name: "Speechmatics",
    base_url: "https://asr.api.speechmatics.com", backend_id: "speechmatics",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #3b82f6, #1d4ed8)",
    default_asr_models: &["enhanced", "standard"],
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
