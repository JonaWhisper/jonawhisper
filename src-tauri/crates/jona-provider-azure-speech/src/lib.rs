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

/// Azure Cognitive Services Speech-to-Text REST API v3.2.
pub struct AzureSpeechBackend;

/// Azure regions available for Speech Services.
const AZURE_REGIONS: &[(&str, &str)] = &[
    ("eastus", "East US"),
    ("eastus2", "East US 2"),
    ("westus", "West US"),
    ("westus2", "West US 2"),
    ("westeurope", "West Europe"),
    ("northeurope", "North Europe"),
    ("southeastasia", "Southeast Asia"),
    ("eastasia", "East Asia"),
    ("japaneast", "Japan East"),
    ("australiaeast", "Australia East"),
    ("centralindia", "Central India"),
    ("canadacentral", "Canada Central"),
    ("uksouth", "UK South"),
    ("francecentral", "France Central"),
    ("switzerlandnorth", "Switzerland North"),
];

impl CloudProvider for AzureSpeechBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        _model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        let api_key = provider.api_key.trim();
        if api_key.is_empty() {
            return Err(ProviderError::NotConfigured(
                "API key is not configured".into(),
            ));
        }

        let region = provider
            .extra
            .get("region")
            .map(|s| s.as_str())
            .unwrap_or("eastus");

        // Validate region against known list to prevent hostname injection
        if !AZURE_REGIONS.iter().any(|(id, _)| *id == region) {
            return Err(ProviderError::NotConfigured(
                "Invalid or missing Azure region".into(),
            ));
        }

        let url = format!(
            "https://{}.api.cognitive.microsoft.com/speechtotext/transcriptions:transcribe?api-version=2024-11-15",
            region
        );

        let file_bytes = std::fs::read(audio_path)?;
        let file_name = audio_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Azure locale format: "fr-FR", "en-US", etc.
        // If language is a 2-letter code, try to expand; otherwise use as-is.
        let locale = azure_locale(language);

        let definition = serde_json::json!({
            "locales": [locale],
            "profanityFilterMode": "None",
        });

        let audio_part = reqwest::blocking::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let definition_part = reqwest::blocking::multipart::Part::text(definition.to_string())
            .mime_str("application/json")
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let form = reqwest::blocking::multipart::Form::new()
            .part("audio", audio_part)
            .part("definition", definition_part);

        let response = BLOCKING_CLIENT
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", api_key)
            .multipart(form)
            .send()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_else(|e| {
                log::warn!("Failed to decode Azure Speech error body: {e}");
                String::new()
            });
            return Err(ProviderError::Api { status, body });
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        // Response: { "combinedPhrases": [{ "text": "..." }] }
        let text = json
            .pointer("/combinedPhrases/0/text")
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
        _provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            // Azure Speech-to-Text REST API doesn't expose model selection
            Ok(vec!["default".into()])
        })
    }
}

/// Convert a 2-letter language code to an Azure BCP-47 locale.
/// Falls back to "{code}-{CODE}" for unknown codes, or passes through
/// values that already look like a locale (contain a hyphen).
fn azure_locale(lang: &str) -> String {
    if lang == "auto" {
        log::info!("Azure Speech: language \"auto\" not supported, falling back to en-US");
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
    backend_id: "azure-speech",
    factory: || Box::new(AzureSpeechBackend),
}}

inventory::submit! { ProviderPreset {
    id: "azure-speech", display_name: "Azure Speech",
    base_url: "https://eastus.api.cognitive.microsoft.com", backend_id: "azure-speech",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #0078d4, #00bcf2)",
    default_asr_models: &["default"],
    default_llm_models: &[],
    extra_fields: &[
        PresetField {
            id: "api_key",
            label: "Subscription Key",
            field_type: FieldType::Password,
            required: true,
            placeholder: "",
            default_value: "",
            options: &[],
            sensitive: true,
        },
        PresetField {
            id: "region",
            label: "Region",
            field_type: FieldType::Select,
            required: true,
            placeholder: "",
            default_value: "eastus",
            options: AZURE_REGIONS,
            sensitive: false,
        },
    ],
    hidden_fields: &[],
}}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azure_locale_two_letter() {
        assert_eq!(azure_locale("fr"), "fr-FR");
        assert_eq!(azure_locale("en"), "en-US");
        assert_eq!(azure_locale("pt"), "pt-BR");
    }

    #[test]
    fn azure_locale_passthrough() {
        assert_eq!(azure_locale("fr-CA"), "fr-CA");
        assert_eq!(azure_locale("en_GB"), "en-GB");
    }

    #[test]
    fn azure_locale_auto_defaults_to_en_us() {
        assert_eq!(azure_locale("auto"), "en-US");
    }

    #[test]
    fn azure_locale_unknown_code() {
        assert_eq!(azure_locale("xx"), "xx-XX");
    }
}
