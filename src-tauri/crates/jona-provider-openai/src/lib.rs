use jona_types::{
    parse_model_ids_from_json, CloudProvider, FieldType, PresetField, Provider,
    ProviderError, ProviderPreset, ProviderRegistration, TranscriptionResult,
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

pub struct OpenAICompatibleBackend;

impl CloudProvider for OpenAICompatibleBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        provider.validate_url().map_err(ProviderError::Http)?;

        let api_key = provider.api_key.trim();

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

        let mut form = reqwest::blocking::multipart::Form::new()
            .part("file", file_part)
            .text("model", model.to_string());

        if language != "auto" {
            form = form.text("language", language.to_string());
        }

        let url = format!("{}/audio/transcriptions", provider.base_url());

        let mut req = BLOCKING_CLIENT.post(&url).multipart(form);
        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {api_key}"));
        }

        let response = req.send().map_err(|e| ProviderError::Http(e.to_string()))?;
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
        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| ProviderError::InvalidResponse(format!(
                "ASR response is not valid JSON: {e}"
            )))?;
        let text = json
            .get("text")
            .and_then(|t| t.as_str())
            .ok_or_else(|| ProviderError::InvalidResponse(
                "ASR response JSON missing 'text' field".into(),
            ))?;
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
            let url = format!("{}/chat/completions", provider.base_url());

            let request = ChatRequest {
                model: model.to_string(),
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

            let api_key = provider.api_key.trim();
            let mut req = ASYNC_CLIENT.post(&url).json(&request);
            if !api_key.is_empty() {
                req = req.header("Authorization", format!("Bearer {api_key}"));
            }

            let response = send_and_check(req).await?;

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
            let url = format!("{}/models", provider.base_url());

            let api_key = provider.api_key.trim();
            let mut req = ASYNC_CLIENT.get(&url);
            if !api_key.is_empty() {
                req = req.header("Authorization", format!("Bearer {api_key}"));
            }

            let response = send_and_check(req).await?;
            let json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;
            parse_model_ids_from_json(&json)
        })
    }
}

// -- Request/Response types --

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
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

async fn send_and_check(req: reqwest::RequestBuilder) -> Result<reqwest::Response, ProviderError> {
    let response = req
        .send()
        .await
        .map_err(|e| ProviderError::Http(e.to_string()))?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(ProviderError::Api { status, body });
    }
    Ok(response)
}

// Backend registration (one instance handles all OpenAI-compatible providers)
inventory::submit! { ProviderRegistration {
    backend_id: "openai",
    factory: || Box::new(OpenAICompatibleBackend),
}}

// Provider presets

// Custom (OpenAI-compatible) — user provides their own URL
inventory::submit! { ProviderPreset {
    id: "openai-compatible", display_name: "Custom (OpenAI-compatible)",
    base_url: "", backend_id: "openai",
    supports_asr: true, supports_llm: true,
    gradient: "linear-gradient(135deg, #6b7280, #4b5563)",
    default_asr_models: &[],
    default_llm_models: &[],
    extra_fields: &[
        PresetField {
            id: "base_url", label: "URL", field_type: FieldType::Text,
            required: true, placeholder: "https://api.example.com/v1", default_value: "",
            options: &[], sensitive: false,
        },
        PresetField {
            id: "api_key", label: "API Key", field_type: FieldType::Password,
            required: false, placeholder: "sk-...", default_value: "",
            options: &[], sensitive: true,
        },
        PresetField {
            id: "supports_asr", label: "Speech-to-Text (ASR)", field_type: FieldType::Toggle,
            required: false, placeholder: "", default_value: "true",
            options: &[], sensitive: false,
        },
        PresetField {
            id: "supports_llm", label: "Text Generation (LLM)", field_type: FieldType::Toggle,
            required: false, placeholder: "", default_value: "true",
            options: &[], sensitive: false,
        },
        PresetField {
            id: "allow_insecure", label: "Allow HTTP (insecure)", field_type: FieldType::Toggle,
            required: false, placeholder: "", default_value: "false",
            options: &[], sensitive: false,
        },
    ],
    hidden_fields: &[],
}}

inventory::submit! { ProviderPreset {
    id: "openai", display_name: "OpenAI",
    base_url: "https://api.openai.com/v1", backend_id: "openai",
    supports_asr: true, supports_llm: true,
    gradient: "linear-gradient(135deg, #10a37f, #0d8c6d)",
    default_asr_models: &["whisper-1", "gpt-4o-transcribe", "gpt-4o-mini-transcribe"],
    default_llm_models: &["gpt-4o-mini", "gpt-4o"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "sk-...", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "groq", display_name: "Groq",
    base_url: "https://api.groq.com/openai/v1", backend_id: "openai",
    supports_asr: true, supports_llm: true,
    gradient: "linear-gradient(135deg, #9333ea, #7c3aed)",
    default_asr_models: &["whisper-large-v3-turbo", "whisper-large-v3"],
    default_llm_models: &["llama-3.1-8b-instant"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "gsk_...", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "cerebras", display_name: "Cerebras",
    base_url: "https://api.cerebras.ai/v1", backend_id: "openai",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #3b82f6, #2563eb)",
    default_asr_models: &[],
    default_llm_models: &["llama3.1-8b"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "gemini", display_name: "Google Gemini",
    base_url: "https://generativelanguage.googleapis.com/v1beta/openai", backend_id: "openai",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #0ea5e9, #0284c7)",
    default_asr_models: &[],
    default_llm_models: &["gemini-2.5-flash-lite"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "AIza...", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "mistral", display_name: "Mistral",
    base_url: "https://api.mistral.ai/v1", backend_id: "openai",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #6366f1, #4f46e5)",
    default_asr_models: &[],
    default_llm_models: &["ministral-3b-latest"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "fireworks", display_name: "Fireworks AI",
    base_url: "https://api.fireworks.ai/inference/v1", backend_id: "openai",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #ef4444, #dc2626)",
    default_asr_models: &["whisper-v3-turbo", "whisper-v3"],
    default_llm_models: &[],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "fw_...", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "together", display_name: "Together AI",
    base_url: "https://api.together.xyz/v1", backend_id: "openai",
    supports_asr: true, supports_llm: true,
    gradient: "linear-gradient(135deg, #14b8a6, #0d9488)",
    default_asr_models: &["openai/whisper-large-v3"],
    default_llm_models: &["meta-llama/Llama-3.2-3B"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "deepseek", display_name: "DeepSeek",
    base_url: "https://api.deepseek.com/v1", backend_id: "openai",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #06b6d4, #0891b2)",
    default_asr_models: &[],
    default_llm_models: &["deepseek-v3.2"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "sk-...", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "openrouter", display_name: "OpenRouter",
    base_url: "https://openrouter.ai/api/v1", backend_id: "openai",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #8b5cf6, #7c3aed)",
    default_asr_models: &[],
    default_llm_models: &["openai/gpt-4o", "anthropic/claude-sonnet-4", "google/gemini-2.0-flash-001"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "sk-or-...", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "xai", display_name: "xAI",
    base_url: "https://api.x.ai/v1", backend_id: "openai",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #1d1d1f, #3a3a3c)",
    default_asr_models: &[],
    default_llm_models: &["grok-2"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "xai-...", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "sambanova", display_name: "SambaNova",
    base_url: "https://api.sambanova.ai/v1", backend_id: "openai",
    supports_asr: true, supports_llm: true,
    gradient: "linear-gradient(135deg, #f97316, #ea580c)",
    default_asr_models: &["whisper-large-v3"],
    default_llm_models: &["Meta-Llama-3.1-8B-Instant"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
inventory::submit! { ProviderPreset {
    id: "nebius", display_name: "Nebius AI",
    base_url: "https://api.studio.nebius.com/v1", backend_id: "openai",
    supports_asr: false, supports_llm: true,
    gradient: "linear-gradient(135deg, #d946ef, #c026d3)",
    default_asr_models: &[],
    default_llm_models: &["meta-llama/Meta-Llama-3.1-8B-Instruct"],
    extra_fields: &[PresetField {
        id: "api_key", label: "API Key", field_type: FieldType::Password,
        required: true, placeholder: "", default_value: "",
        options: &[], sensitive: true,
    }],
    hidden_fields: &[],
}}
