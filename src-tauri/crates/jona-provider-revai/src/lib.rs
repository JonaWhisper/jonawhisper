use jona_types::{
    CloudProvider, Provider, ProviderError, ProviderPreset, ProviderRegistration,
    TranscriptionResult,
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

/// Rev.ai ASR — multipart upload to the synchronous speech-to-text endpoint.
pub struct RevAiBackend;

impl CloudProvider for RevAiBackend {
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
                "Provider '{}' is missing an API key for Rev.ai",
                provider.name
            )));
        }

        let file_bytes = std::fs::read(audio_path)?;
        let file_part = reqwest::blocking::multipart::Part::bytes(file_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let mut form = reqwest::blocking::multipart::Form::new().part("media", file_part);
        if !model.is_empty() {
            form = form.text("model", model.to_string());
        }
        if language != "auto" {
            form = form.text("language", language.to_string());
        }

        let url = format!("{}/speechtotext/v1/jobs", provider.base_url());

        let response = BLOCKING_CLIENT
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .multipart(form)
            .send()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(ProviderError::Api { status, body });
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        // Rev.ai response: monologues[].elements[].value concatenated
        let text = extract_revai_text(&json);

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
                "reverb-english".into(),
                "reverb-foreign".into(),
            ])
        })
    }
}

/// Extract transcript text from Rev.ai JSON response.
/// Structure: { monologues: [{ elements: [{ type: "text"|"punct", value: "..." }] }] }
fn extract_revai_text(json: &serde_json::Value) -> String {
    let mut result = String::new();
    if let Some(monologues) = json.get("monologues").and_then(|v| v.as_array()) {
        for monologue in monologues {
            if let Some(elements) = monologue.get("elements").and_then(|v| v.as_array()) {
                for element in elements {
                    if let Some(value) = element.get("value").and_then(|v| v.as_str()) {
                        result.push_str(value);
                    }
                }
            }
        }
    }
    result.trim().to_string()
}

inventory::submit! { ProviderRegistration {
    backend_id: "revai",
    factory: || Box::new(RevAiBackend),
}}

inventory::submit! { ProviderPreset {
    id: "revai", display_name: "Rev.ai",
    base_url: "https://api.rev.ai", backend_id: "revai",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #0066ff, #0044cc)",
    default_asr_models: &["reverb-english", "reverb-foreign"],
    default_llm_models: &[],
}}
