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

/// Google Cloud Speech-to-Text v1 REST API (sync recognize).
pub struct GoogleSpeechBackend;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecognizeRequest {
    config: RecognitionConfig,
    audio: RecognitionAudio,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecognitionConfig {
    encoding: &'static str,
    sample_rate_hertz: u32,
    language_code: String,
    model: String,
    enable_automatic_punctuation: bool,
}

#[derive(Serialize)]
struct RecognitionAudio {
    content: String,
}

impl CloudProvider for GoogleSpeechBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        if provider.api_key.trim().is_empty() {
            return Err(ProviderError::NotConfigured(
                "API key is not configured".into(),
            ));
        }

        let file_bytes = std::fs::read(audio_path)?;

        // Read sample rate from WAV header (bytes 24-27, little-endian u32)
        let sample_rate = if file_bytes.len() >= 28 {
            u32::from_le_bytes([
                file_bytes[24],
                file_bytes[25],
                file_bytes[26],
                file_bytes[27],
            ])
        } else {
            16000
        };

        let audio_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &file_bytes,
        );

        let language_code = google_language_code(language);

        let request = RecognizeRequest {
            config: RecognitionConfig {
                // Recording pipeline always produces 16-bit PCM WAV (see audio.rs)
                encoding: "LINEAR16",
                sample_rate_hertz: sample_rate,
                language_code,
                model: model.to_string(),
                enable_automatic_punctuation: true,
            },
            audio: RecognitionAudio { content: audio_b64 },
        };

        let url = reqwest::Url::parse_with_params(
            "https://speech.googleapis.com/v1/speech:recognize",
            &[("key", &provider.api_key)],
        )
        .map_err(|e| ProviderError::Http(e.to_string()))?;

        let response = BLOCKING_CLIENT
            .post(url)
            .json(&request)
            .send()
            .map_err(|e| {
                let msg = e.to_string().replace(&provider.api_key, "[REDACTED]");
                ProviderError::Http(msg)
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            return Err(ProviderError::Api { status, body });
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        // Response: { "results": [{ "alternatives": [{ "transcript": "..." }] }] }
        // Concatenate all results' first alternative transcripts.
        let text = json
            .get("results")
            .and_then(|r| r.as_array())
            .map(|results| {
                results
                    .iter()
                    .filter_map(|r| {
                        r.pointer("/alternatives/0/transcript")
                            .and_then(|v| v.as_str())
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();

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
                "latest_long".into(),
                "latest_short".into(),
                "telephony".into(),
                "medical_dictation".into(),
                "medical_conversation".into(),
            ])
        })
    }
}

/// Convert a 2-letter ISO 639-1 code to a Google BCP-47 language code.
fn google_language_code(lang: &str) -> String {
    if lang == "auto" {
        // Google doesn't have a true "auto" — default to en-US
        return "en-US".to_string();
    }
    if lang.contains('-') || lang.contains('_') {
        return lang.replace('_', "-");
    }
    match lang {
        "en" => "en-US".to_string(),
        "fr" => "fr-FR".to_string(),
        "de" => "de-DE".to_string(),
        "es" => "es-ES".to_string(),
        "it" => "it-IT".to_string(),
        "pt" => "pt-BR".to_string(),
        "nl" => "nl-NL".to_string(),
        "pl" => "pl-PL".to_string(),
        "ru" => "ru-RU".to_string(),
        "ja" => "ja-JP".to_string(),
        "ko" => "ko-KR".to_string(),
        "zh" => "zh-CN".to_string(),
        "ar" => "ar-SA".to_string(),
        "hi" => "hi-IN".to_string(),
        "sv" => "sv-SE".to_string(),
        "da" => "da-DK".to_string(),
        "fi" => "fi-FI".to_string(),
        "nb" => "nb-NO".to_string(),
        "tr" => "tr-TR".to_string(),
        "uk" => "uk-UA".to_string(),
        "cs" => "cs-CZ".to_string(),
        "el" => "el-GR".to_string(),
        "ro" => "ro-RO".to_string(),
        "hu" => "hu-HU".to_string(),
        "th" => "th-TH".to_string(),
        "vi" => "vi-VN".to_string(),
        "id" => "id-ID".to_string(),
        "ms" => "ms-MY".to_string(),
        code => format!("{}-{}", code, code.to_uppercase()),
    }
}

inventory::submit! { ProviderRegistration {
    backend_id: "google-speech",
    factory: || Box::new(GoogleSpeechBackend),
}}

inventory::submit! { ProviderPreset {
    id: "google-speech", display_name: "Google Cloud Speech",
    base_url: "https://speech.googleapis.com", backend_id: "google-speech",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #34a853, #4285f4)",
    default_asr_models: &["latest_long", "latest_short"],
    default_llm_models: &[],
    extra_fields: &[],
    hidden_fields: &["base_url"],
}}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn google_locale_two_letter() {
        assert_eq!(google_language_code("fr"), "fr-FR");
        assert_eq!(google_language_code("en"), "en-US");
        assert_eq!(google_language_code("pt"), "pt-BR");
    }

    #[test]
    fn google_locale_passthrough() {
        assert_eq!(google_language_code("fr-CA"), "fr-CA");
        assert_eq!(google_language_code("en_GB"), "en-GB");
    }

    #[test]
    fn google_locale_auto() {
        assert_eq!(google_language_code("auto"), "en-US");
    }

    #[test]
    fn google_locale_unknown() {
        assert_eq!(google_language_code("xx"), "xx-XX");
    }
}
