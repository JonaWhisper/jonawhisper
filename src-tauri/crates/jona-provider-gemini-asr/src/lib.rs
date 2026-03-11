use jona_types::{
    CloudProvider, Provider, ProviderError, ProviderPreset, ProviderRegistration,
    TranscriptionResult,
};
use serde::Serialize;
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

/// Gemini native ASR — sends audio as base64 inline_data to the generateContent API.
pub struct GeminiAsrBackend;

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

#[derive(Serialize)]
struct InlineData {
    mime_type: &'static str,
    data: String,
}

impl CloudProvider for GeminiAsrBackend {
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
                "Provider '{}' is missing an API key for Gemini",
                provider.name
            )));
        }

        let file_bytes = std::fs::read(audio_path)?;
        let audio_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &file_bytes,
        );

        let prompt = if language != "auto" {
            format!(
                "Transcribe the following audio. Output only the transcription text, nothing else. Language: {}",
                language
            )
        } else {
            "Transcribe the following audio. Output only the transcription text, nothing else.".to_string()
        };

        let request = GeminiRequest {
            contents: vec![GeminiContent {
                parts: vec![
                    GeminiPart::InlineData {
                        inline_data: InlineData {
                            mime_type: "audio/wav",
                            data: audio_b64,
                        },
                    },
                    GeminiPart::Text { text: prompt },
                ],
            }],
        };

        let base_url = provider.base_url();
        let url = format!(
            "{}/models/{}:generateContent",
            base_url.trim_end_matches('/'),
            model,
        );

        let response = BLOCKING_CLIENT
            .post(&url)
            .header("x-goog-api-key", api_key)
            .json(&request)
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

        // Response: candidates[0].content.parts[0].text
        let text = json
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
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
                "Provider '{}' (Gemini ASR) does not support LLM chat — use the Gemini OpenAI-compat preset for LLM",
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
                "gemini-2.0-flash".into(),
                "gemini-2.5-flash".into(),
                "gemini-2.5-pro".into(),
            ])
        })
    }
}

inventory::submit! { ProviderRegistration {
    backend_id: "gemini-asr",
    factory: || Box::new(GeminiAsrBackend),
}}

inventory::submit! { ProviderPreset {
    id: "gemini-asr", display_name: "Gemini ASR",
    base_url: "https://generativelanguage.googleapis.com/v1beta", backend_id: "gemini-asr",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #4285f4, #1a73e8)",
    default_asr_models: &["gemini-2.0-flash", "gemini-2.5-flash"],
    default_llm_models: &[],
}}
