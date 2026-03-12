use jona_types::{
    CloudProvider, FieldType, PresetField, Provider, ProviderError, ProviderPreset,
    ProviderRegistration, TranscriptionResult,
};
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

/// ElevenLabs Scribe — multipart sync transcription endpoint.
pub struct ElevenLabsBackend;

impl CloudProvider for ElevenLabsBackend {
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
                "Provider '{}' is missing an API key for ElevenLabs",
                provider.name
            )));
        }

        let file_bytes = std::fs::read(audio_path)?;
        let file_part = reqwest::blocking::multipart::Part::bytes(file_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let mut form = reqwest::blocking::multipart::Form::new()
            .part("file", file_part)
            .text("model_id", model.to_string());

        if language != "auto" {
            form = form.text("language_code", language.to_string());
        }

        let url = format!("{}/v1/speech-to-text", provider.base_url());

        let response = BLOCKING_CLIENT
            .post(&url)
            .header("xi-api-key", api_key)
            .multipart(form)
            .send()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_else(|e| {
                log::warn!("ElevenLabs: failed to decode error body: {e}");
                String::new()
            });
            return Err(ProviderError::Api { status, body });
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        let text = json
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ProviderError::InvalidResponse(
                    "Missing transcript in ElevenLabs response".into(),
                )
            })?
            .to_string();

        Ok(TranscriptionResult::text_only(text))
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
                "scribe_v1".into(),
                "scribe_v2".into(),
            ])
        })
    }
}

inventory::submit! { ProviderRegistration {
    backend_id: "elevenlabs",
    factory: || Box::new(ElevenLabsBackend),
}}

inventory::submit! { ProviderPreset {
    id: "elevenlabs", display_name: "ElevenLabs",
    base_url: "https://api.elevenlabs.io", backend_id: "elevenlabs",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #f97316, #ea580c)",
    default_asr_models: &["scribe_v2", "scribe_v1"],
    default_llm_models: &[],
    extra_fields: &[
        PresetField {
            id: "api_key",
            label: "API Key",
            field_type: FieldType::Password,
            required: true,
            placeholder: "xi_...",
            default_value: "",
            options: &[],
            sensitive: true,
        },
    ],
    hidden_fields: &[],
}}
