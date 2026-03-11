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

pub struct DeepgramBackend;

impl CloudProvider for DeepgramBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        provider.validate_url().map_err(ProviderError::Http)?;

        let file_bytes = std::fs::read(audio_path)?;

        let mut url = format!(
            "{}/v1/listen?model={}&smart_format=true",
            provider.base_url(),
            model,
        );
        if language != "auto" {
            url.push_str(&format!("&language={}", language));
        }

        let mut req = BLOCKING_CLIENT
            .post(&url)
            .header("Content-Type", "audio/wav")
            .body(file_bytes);
        if !provider.api_key.is_empty() {
            req = req.header("Authorization", format!("Token {}", provider.api_key));
        }

        let response = req.send().map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(ProviderError::Api { status, body });
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        // Deepgram response: results.channels[0].alternatives[0].transcript
        let text = json
            .pointer("/results/channels/0/alternatives/0/transcript")
            .and_then(|v| v.as_str())
            .unwrap_or("")
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
        provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            // Deepgram has no /models endpoint — return known models
            let _ = provider;
            Ok(vec![
                "nova-3".into(),
                "nova-2".into(),
                "enhanced".into(),
                "base".into(),
            ])
        })
    }
}

inventory::submit! { ProviderRegistration {
    backend_id: "deepgram",
    factory: || Box::new(DeepgramBackend),
}}

inventory::submit! { ProviderPreset {
    id: "deepgram", display_name: "Deepgram",
    base_url: "https://api.deepgram.com", backend_id: "deepgram",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #13ef93, #149e6a)",
    default_asr_models: &["nova-3", "nova-2"],
    default_llm_models: &[],
    extra_fields: &[],
    hidden_fields: &[],
}}
