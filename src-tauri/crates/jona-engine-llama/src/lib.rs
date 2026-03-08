mod inference;

use jona_types::{
    ASREngine, ASRModel, DownloadType, EngineCategory, EngineError, EngineRegistration,
    GpuMode, Language, common_languages,
};
use std::any::Any;

pub use inference::{LlmContext, cleanup_text};

pub struct LlamaEngine;

impl ASREngine for LlamaEngine {
    fn engine_id(&self) -> &str { "llama" }
    fn display_name(&self) -> &str { "Llama" }
    fn category(&self) -> EngineCategory { EngineCategory::LLM }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "llama:qwen3-0.6b".into(), engine_id: "llama".into(),
                label: "Qwen3".into(),
                filename: "Qwen_Qwen3-0.6B-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/Qwen_Qwen3-0.6B-GGUF/resolve/main/Qwen_Qwen3-0.6B-Q4_K_M.gguf".into(),
                size: 484_000_000, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(0.6), ram: Some(600_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "es".into(), "de".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:gemma3-1b".into(), engine_id: "llama".into(),
                label: "Gemma 3".into(),
                filename: "google_gemma-3-1b-it-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/google_gemma-3-1b-it-GGUF/resolve/main/google_gemma-3-1b-it-Q4_K_M.gguf".into(),
                size: 806_058_496, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(1.0), ram: Some(1_000_000_000),
                lang_codes: Some(vec!["en".into(), "fr".into(), "es".into(), "de".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:llama3.2-1b".into(), engine_id: "llama".into(),
                label: "Llama 3.2".into(),
                filename: "Llama-3.2-1B-Instruct-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf".into(),
                size: 808_000_000, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(1.0), ram: Some(1_000_000_000),
                lang_codes: Some(vec!["en".into(), "es".into(), "de".into(), "fr".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:smollm2-1.7b".into(), engine_id: "llama".into(),
                label: "SmolLM2".into(),
                filename: "SmolLM2-1.7B-Instruct-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/SmolLM2-1.7B-Instruct-GGUF/resolve/main/SmolLM2-1.7B-Instruct-Q4_K_M.gguf".into(),
                size: 1_055_609_824, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(1.7), ram: Some(1_300_000_000),
                lang_codes: Some(vec!["en".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:qwen3-1.7b".into(), engine_id: "llama".into(),
                label: "Qwen3".into(),
                filename: "Qwen_Qwen3-1.7B-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/Qwen_Qwen3-1.7B-GGUF/resolve/main/Qwen_Qwen3-1.7B-Q4_K_M.gguf".into(),
                size: 1_282_439_584, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                recommended_for: Some(vec![]),
                params: Some(1.7), ram: Some(1_500_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "es".into(), "de".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:smollm3-3b".into(), engine_id: "llama".into(),
                label: "SmolLM3".into(),
                filename: "HuggingFaceTB_SmolLM3-3B-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/HuggingFaceTB_SmolLM3-3B-GGUF/resolve/main/HuggingFaceTB_SmolLM3-3B-Q4_K_M.gguf".into(),
                size: 1_920_000_000, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(3.0), ram: Some(2_300_000_000),
                lang_codes: Some(vec!["en".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:llama3.2-3b".into(), engine_id: "llama".into(),
                label: "Llama 3.2".into(),
                filename: "Llama-3.2-3B-Instruct-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf".into(),
                size: 2_020_000_000, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(3.0), ram: Some(2_500_000_000),
                lang_codes: Some(vec!["en".into(), "es".into(), "de".into(), "fr".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:ministral3-3b".into(), engine_id: "llama".into(),
                label: "Ministral 3".into(),
                filename: "mistralai_Ministral-3-3B-Instruct-2512-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/mistralai_Ministral-3-3B-Instruct-2512-GGUF/resolve/main/mistralai_Ministral-3-3B-Instruct-2512-Q4_K_M.gguf".into(),
                size: 2_150_000_000, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(3.0), ram: Some(2_500_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "es".into(), "de".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:gemma3-4b".into(), engine_id: "llama".into(),
                label: "Gemma 3".into(),
                filename: "google_gemma-3-4b-it-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/google_gemma-3-4b-it-GGUF/resolve/main/google_gemma-3-4b-it-Q4_K_M.gguf".into(),
                size: 2_490_000_000, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(4.0), ram: Some(3_000_000_000),
                lang_codes: Some(vec!["en".into(), "fr".into(), "es".into(), "de".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:phi4-mini".into(), engine_id: "llama".into(),
                label: "Phi-4 Mini".into(),
                filename: "microsoft_Phi-4-mini-instruct-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/microsoft_Phi-4-mini-instruct-GGUF/resolve/main/microsoft_Phi-4-mini-instruct-Q4_K_M.gguf".into(),
                size: 2_491_874_688, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(3.8), ram: Some(3_000_000_000),
                lang_codes: Some(vec!["en".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
            ASRModel {
                id: "llama:qwen3-4b".into(), engine_id: "llama".into(),
                label: "Qwen3".into(),
                filename: "Qwen_Qwen3-4B-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/Qwen_Qwen3-4B-GGUF/resolve/main/Qwen_Qwen3-4B-Q4_K_M.gguf".into(),
                size: 2_497_280_960, storage_dir: jona_types::engine_storage_dir("llm"),
                download_type: DownloadType::SingleFile, download_marker: None,
                params: Some(4.0), ram: Some(3_000_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "es".into(), "de".into()]),
                quantization: Some("Q4".into()),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> { common_languages() }

    fn description(&self) -> &str {
        if cfg!(target_os = "macos") {
            "Local LLM inference via llama.cpp with Metal GPU acceleration."
        } else {
            "Local LLM inference via llama.cpp with CPU."
        }
    }

    fn create_context(&self, model: &ASRModel, _gpu_mode: GpuMode)
        -> Result<Box<dyn Any + Send>, EngineError>
    {
        let ctx = LlmContext::load(&model.local_path())
            .map_err(|e| EngineError::LaunchFailed(e.to_string()))?;
        Ok(Box::new(ctx))
    }

    fn cleanup(&self, ctx: &mut dyn Any, text: &str, language: &str, max_tokens: usize)
        -> Result<String, EngineError>
    {
        let ctx = ctx.downcast_ref::<LlmContext>()
            .ok_or_else(|| EngineError::LaunchFailed("Invalid LLM context".into()))?;
        cleanup_text(ctx, text, language, max_tokens)
            .map_err(|e| EngineError::LaunchFailed(e.to_string()))
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(LlamaEngine) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jona_types::ASREngine;

    #[test]
    fn engine_registers_as_llm() {
        // Llama is a local LLM for text cleanup, shown in the cleanup model picker.
        let engine = LlamaEngine;
        assert_eq!(engine.engine_id(), "llama");
        assert_eq!(engine.category(), jona_types::EngineCategory::LLM);
    }

    #[test]
    fn user_can_pick_at_least_one_model() {
        let engine = LlamaEngine;
        assert!(!engine.models().is_empty(), "User must be able to choose at least one Llama model");
    }

    #[test]
    fn no_duplicate_models_in_picker() {
        let engine = LlamaEngine;
        let models = engine.models();
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count, "Duplicate models would confuse the user");
    }

    #[test]
    fn all_download_urls_are_secure() {
        let engine = LlamaEngine;
        for model in engine.models() {
            if !model.url.is_empty() {
                assert!(model.url.starts_with("https://"),
                    "Model {} has insecure download URL: {}", model.id, model.url);
            }
        }
    }

    #[test]
    fn models_report_size_for_download_progress() {
        let engine = LlamaEngine;
        for model in engine.models() {
            assert!(model.size > 0,
                "Model {} reports zero size, download progress UI would be broken", model.id);
        }
    }

    #[test]
    fn models_report_ram_requirement() {
        // The UI shows RAM requirements so the user can pick a model that fits their machine.
        let engine = LlamaEngine;
        for model in engine.models() {
            assert!(model.ram.is_some() && model.ram.unwrap() > 0,
                "Model {} should report RAM requirement for the UI", model.id);
        }
    }
}
