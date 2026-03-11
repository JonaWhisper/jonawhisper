use jona_types::{
    CloudProvider, FieldType, PresetField, Provider, ProviderError, ProviderPreset,
    ProviderRegistration, TranscriptionResult,
};
use serde::{Deserialize, Serialize};
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

static ASYNC_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Azure OpenAI Service backend.
///
/// Uses the OpenAI-compatible API with Azure-specific endpoint format:
/// `https://{resource}.openai.azure.com/openai/deployments/{deployment}/...`
pub struct AzureOpenAIBackend;

/// Extract a required extra field, returning `ProviderError::NotConfigured` if missing/empty.
fn required_extra<'a>(provider: &'a Provider, key: &str) -> Result<&'a str, ProviderError> {
    provider
        .extra
        .get(key)
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ProviderError::NotConfigured(format!("Missing required field: {key}"))
        })
}

/// Validate that a URL segment contains only safe characters (alphanumeric, hyphens, dots).
/// Used for resource_name, deployment_name, and api_version to prevent path/query injection.
fn validate_url_segment(value: &str, field_name: &str) -> Result<(), ProviderError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.starts_with('.')
        || value.ends_with('.')
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        return Err(ProviderError::NotConfigured(format!(
            "Invalid {field_name}: must contain only alphanumeric characters, hyphens and dots, \
             and must not start/end with a dot"
        )));
    }
    Ok(())
}

/// Resolve the deployment name: use `model` parameter if non-empty, otherwise fall back
/// to `extra.deployment_name`. This lets the UI model picker override the default.
fn resolve_deployment<'a>(provider: &'a Provider, model: &'a str) -> Result<&'a str, ProviderError> {
    let deployment = if !model.is_empty() && model != "default" {
        model
    } else {
        required_extra(provider, "deployment_name")?
    };
    validate_url_segment(deployment, "deployment_name")?;
    Ok(deployment)
}

impl CloudProvider for AzureOpenAIBackend {
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
                "API key is not configured".into(),
            ));
        }

        let resource = required_extra(provider, "resource_name")?;
        validate_url_segment(resource, "resource_name")?;
        let deployment = resolve_deployment(provider, model)?;
        let api_version = provider
            .extra
            .get("api_version")
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("2024-10-21");
        validate_url_segment(api_version, "api_version")?;

        let url = format!(
            "https://{resource}.openai.azure.com/openai/deployments/{deployment}/audio/transcriptions?api-version={api_version}"
        );

        let file_bytes = std::fs::read(audio_path)?;
        let file_name = audio_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let file_part = reqwest::blocking::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        // Azure OpenAI Whisper uses the same multipart format as OpenAI
        let mut form = reqwest::blocking::multipart::Form::new()
            .part("file", file_part)
            .text("model", deployment.to_string());

        if language != "auto" {
            form = form.text("language", language.to_string());
        }

        let response = BLOCKING_CLIENT
            .post(&url)
            .header("api-key", api_key)
            .multipart(form)
            .send()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = response.status();

        let body = response
            .text()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !status.is_success() {
            return Err(ProviderError::Api {
                status: status.as_u16(),
                body,
            });
        }
        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            ProviderError::InvalidResponse(format!("ASR response is not valid JSON: {e}"))
        })?;
        let text = json
            .get("text")
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                ProviderError::InvalidResponse("ASR response JSON missing 'text' field".into())
            })?;
        Ok(TranscriptionResult::text_only(text.to_string()))
    }

    fn chat_completion<'a>(
        &'a self,
        provider: &'a Provider,
        model: &'a str,
        system: &'a str,
        user_message: &'a str,
        temperature: f32,
        max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let api_key = provider.api_key.trim();
            if api_key.is_empty() {
                return Err(ProviderError::NotConfigured(
                    "API key is not configured".into(),
                ));
            }

            let resource = required_extra(provider, "resource_name")?;
            validate_url_segment(resource, "resource_name")?;
            let deployment = resolve_deployment(provider, model)?;
            let api_version = provider
                .extra
                .get("api_version")
                .map(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("2024-10-21");
            validate_url_segment(api_version, "api_version")?;

            let url = format!(
                "https://{resource}.openai.azure.com/openai/deployments/{deployment}/chat/completions?api-version={api_version}"
            );

            let request = ChatRequest {
                messages: vec![
                    ChatMessage {
                        role: "system",
                        content: system.to_string(),
                    },
                    ChatMessage {
                        role: "user",
                        content: user_message.to_string(),
                    },
                ],
                temperature,
                max_tokens,
            };

            let response = ASYNC_CLIENT
                .post(&url)
                .header("api-key", api_key)
                .json(&request)
                .send()
                .await
                .map_err(|e| ProviderError::Http(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_else(|e| {
                    log::warn!("Failed to decode error response body: {e}");
                    String::new()
                });
                return Err(ProviderError::Api { status, body });
            }

            let chat_response: ChatResponse = response
                .json()
                .await
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

            chat_response
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content.trim().to_string())
                .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".into()))
        })
    }

    fn list_models<'a>(
        &'a self,
        provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            // Azure OpenAI doesn't have a model listing API — models are tied to deployments.
            // Return the deployment name as the only available model.
            let deployment = required_extra(provider, "deployment_name")?.trim();
            if deployment.is_empty() {
                return Err(ProviderError::NotConfigured(
                    "deployment_name is not configured".into(),
                ));
            }
            validate_url_segment(deployment, "deployment_name")?;
            Ok(vec![deployment.to_string()])
        })
    }
}

// -- Request/Response types (no `model` field — Azure infers from deployment) --

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

// -- Registration --

inventory::submit! { ProviderRegistration {
    backend_id: "azure-openai",
    factory: || Box::new(AzureOpenAIBackend),
}}

inventory::submit! { ProviderPreset {
    id: "azure-openai", display_name: "Azure OpenAI",
    base_url: "https://azure.openai.azure.com", backend_id: "azure-openai",
    supports_asr: true, supports_llm: true,
    gradient: "linear-gradient(135deg, #0078d4, #5c2d91)",
    default_asr_models: &[],
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
        PresetField {
            id: "resource_name",
            label: "Resource Name",
            field_type: FieldType::Text,
            required: true,
            placeholder: "my-resource",
            default_value: "",
            options: &[],
            sensitive: false,
        },
        PresetField {
            id: "deployment_name",
            label: "Deployment Name",
            field_type: FieldType::Text,
            required: true,
            placeholder: "gpt-4o",
            default_value: "",
            options: &[],
            sensitive: false,
        },
        PresetField {
            id: "api_version",
            label: "API Version",
            field_type: FieldType::Text,
            required: true,
            placeholder: "2024-10-21",
            default_value: "2024-10-21",
            options: &[],
            sensitive: false,
        },
    ],
    hidden_fields: &["base_url"],
}}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_provider(extra: HashMap<String, String>) -> Provider {
        Provider {
            id: "test".into(),
            name: "Test".into(),
            kind: "azure-openai".into(),
            url: String::new(),
            api_key: "test-key".into(),
            allow_insecure: false,
            cached_models: vec![],
            supports_asr: true,
            supports_llm: true,
            api_format: None,
            extra,
        }
    }

    #[test]
    fn required_extra_returns_value() {
        let mut extra = HashMap::new();
        extra.insert("resource_name".into(), "my-resource".into());
        let provider = test_provider(extra);
        assert_eq!(required_extra(&provider, "resource_name").unwrap(), "my-resource");
    }

    #[test]
    fn required_extra_missing_field() {
        let provider = test_provider(HashMap::new());
        assert!(required_extra(&provider, "resource_name").is_err());
    }

    #[test]
    fn required_extra_empty_field() {
        let mut extra = HashMap::new();
        extra.insert("resource_name".into(), String::new());
        let provider = test_provider(extra);
        assert!(required_extra(&provider, "resource_name").is_err());
    }

    #[test]
    fn validate_url_segment_valid() {
        assert!(validate_url_segment("my-resource", "test").is_ok());
        assert!(validate_url_segment("resource123", "test").is_ok());
        assert!(validate_url_segment("2024-10-21", "test").is_ok());
        assert!(validate_url_segment("gpt-4o", "test").is_ok());
    }

    #[test]
    fn validate_url_segment_invalid() {
        assert!(validate_url_segment("", "test").is_err());
        assert!(validate_url_segment("my resource", "test").is_err());
        assert!(validate_url_segment("name/path", "test").is_err());
        assert!(validate_url_segment("ver?a=1", "test").is_err());
        assert!(validate_url_segment("dep&inject", "test").is_err());
        assert!(validate_url_segment("../escape", "test").is_err());
        assert!(validate_url_segment(".", "test").is_err());
        assert!(validate_url_segment("..", "test").is_err());
        assert!(validate_url_segment(".hidden", "test").is_err());
        assert!(validate_url_segment("trailing.", "test").is_err());
    }

    #[test]
    fn resolve_deployment_uses_model_param() {
        let mut extra = HashMap::new();
        extra.insert("resource_name".into(), "res".into());
        extra.insert("deployment_name".into(), "fallback-dep".into());
        let provider = test_provider(extra);
        assert_eq!(resolve_deployment(&provider, "custom-dep").unwrap(), "custom-dep");
    }

    #[test]
    fn resolve_deployment_falls_back_to_extra() {
        let mut extra = HashMap::new();
        extra.insert("resource_name".into(), "res".into());
        extra.insert("deployment_name".into(), "extra-dep".into());
        let provider = test_provider(extra);
        assert_eq!(resolve_deployment(&provider, "").unwrap(), "extra-dep");
        assert_eq!(resolve_deployment(&provider, "default").unwrap(), "extra-dep");
    }

    #[test]
    fn transcribe_missing_api_key() {
        let mut extra = HashMap::new();
        extra.insert("resource_name".into(), "res".into());
        extra.insert("deployment_name".into(), "dep".into());
        let mut provider = test_provider(extra);
        provider.api_key = String::new();

        let backend = AzureOpenAIBackend;
        let result = backend.transcribe(&provider, "model", Path::new("/nonexistent"), "en");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::NotConfigured(_)));
    }

    #[test]
    fn transcribe_missing_resource_name() {
        let mut extra = HashMap::new();
        extra.insert("deployment_name".into(), "dep".into());
        let provider = test_provider(extra);

        let backend = AzureOpenAIBackend;
        let result = backend.transcribe(&provider, "model", Path::new("/nonexistent"), "en");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::NotConfigured(_)));
    }

    #[test]
    fn list_models_returns_deployment_name() {
        let mut extra = HashMap::new();
        extra.insert("resource_name".into(), "res".into());
        extra.insert("deployment_name".into(), "whisper-deploy".into());
        let provider = test_provider(extra);

        let backend = AzureOpenAIBackend;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let models = rt.block_on(backend.list_models(&provider)).unwrap();
        assert_eq!(models, vec!["whisper-deploy"]);
    }
}
