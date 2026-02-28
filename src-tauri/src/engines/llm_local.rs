use super::*;

pub struct LlmLocalEngine;

const LLM_STORAGE_DIR: &str = "~/.local/share/whisper-dictate/llm";

impl ASREngine for LlmLocalEngine {
    fn engine_id(&self) -> &str { "llm-local" }
    fn display_name(&self) -> &str { "Llama" }
    fn category(&self) -> EngineCategory { EngineCategory::LLM }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "llm-local:qwen3-1.7b".into(), engine_id: "llm-local".into(),
                label: "Qwen3 1.7B".into(),
                filename: "Qwen_Qwen3-1.7B-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/Qwen_Qwen3-1.7B-GGUF/resolve/main/Qwen_Qwen3-1.7B-Q4_K_M.gguf".into(),
                size: 1_282_439_584, storage_dir: LLM_STORAGE_DIR.into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                recommended: true,
                ..Default::default()
            },
            ASRModel {
                id: "llm-local:smollm2-1.7b".into(), engine_id: "llm-local".into(),
                label: "SmolLM2 1.7B".into(),
                filename: "SmolLM2-1.7B-Instruct-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/SmolLM2-1.7B-Instruct-GGUF/resolve/main/SmolLM2-1.7B-Instruct-Q4_K_M.gguf".into(),
                size: 1_055_609_824, storage_dir: LLM_STORAGE_DIR.into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                ..Default::default()
            },
            ASRModel {
                id: "llm-local:gemma3-1b".into(), engine_id: "llm-local".into(),
                label: "Gemma 3 1B".into(),
                filename: "google_gemma-3-1b-it-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/google_gemma-3-1b-it-GGUF/resolve/main/google_gemma-3-1b-it-Q4_K_M.gguf".into(),
                size: 806_058_496, storage_dir: LLM_STORAGE_DIR.into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                ..Default::default()
            },
            ASRModel {
                id: "llm-local:qwen3-4b".into(), engine_id: "llm-local".into(),
                label: "Qwen3 4B".into(),
                filename: "Qwen_Qwen3-4B-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/Qwen_Qwen3-4B-GGUF/resolve/main/Qwen_Qwen3-4B-Q4_K_M.gguf".into(),
                size: 2_497_280_960, storage_dir: LLM_STORAGE_DIR.into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                ..Default::default()
            },
            ASRModel {
                id: "llm-local:phi4-mini".into(), engine_id: "llm-local".into(),
                label: "Phi-4 Mini 3.8B".into(),
                filename: "microsoft_Phi-4-mini-instruct-Q4_K_M.gguf".into(),
                url: "https://huggingface.co/bartowski/microsoft_Phi-4-mini-instruct-GGUF/resolve/main/microsoft_Phi-4-mini-instruct-Q4_K_M.gguf".into(),
                size: 2_491_874_688, storage_dir: LLM_STORAGE_DIR.into(),
                download_type: DownloadType::SingleFile, download_marker: None,
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
}
