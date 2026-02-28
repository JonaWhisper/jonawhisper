pub mod whisper;
pub mod openai_api;
pub mod downloader;
pub mod llm_local;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// -- Engine category --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineCategory {
    ASR,
    LLM,
}

// -- Download type --

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DownloadType {
    #[default]
    SingleFile,
    RemoteAPI,
    System,
}

// -- Model --

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ASRModel {
    pub id: String,
    pub engine_id: String,
    pub label: String,
    pub filename: String,
    pub url: String,
    pub size: u64,
    pub storage_dir: String,
    pub download_type: DownloadType,
    pub download_marker: Option<String>,
    pub wer: Option<f32>,
    pub rtf: Option<f32>,
    #[serde(default)]
    pub recommended: bool,
    /// Number of parameters in billions (for LLM models).
    pub params: Option<f32>,
}

impl ASRModel {
    pub fn local_path(&self) -> PathBuf {
        let expanded = shellexpand::tilde(&self.storage_dir);
        PathBuf::from(expanded.as_ref()).join(&self.filename)
    }

    pub fn is_downloaded(&self) -> bool {
        match &self.download_type {
            DownloadType::RemoteAPI | DownloadType::System => true,
            _ => {
                let path = self.local_path();
                if let Some(marker) = &self.download_marker {
                    path.join(marker).exists()
                } else {
                    path.exists()
                }
            }
        }
    }

}

// -- Language --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Language {
    pub code: String,
    pub label: String,
}

// -- Engine trait --

pub trait ASREngine: Send + Sync {
    fn engine_id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn category(&self) -> EngineCategory { EngineCategory::ASR }
    fn models(&self) -> Vec<ASRModel>;
    fn supported_languages(&self) -> Vec<Language>;
    fn description(&self) -> &str;
    fn recommended_model_id(&self, _language: &str) -> Option<String> {
        self.models().into_iter().find(|m| m.recommended).map(|m| m.id)
    }
}

// -- Engine info (serializable for frontend) --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: EngineCategory,
    pub available: bool,
    pub supported_language_codes: Vec<String>,
}

// -- Errors --

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Model not found at {0}")]
    ModelNotFound(String),
    #[error("Failed to launch: {0}")]
    LaunchFailed(String),
    #[error("API error: {0}")]
    ApiError(String),
}

impl Serialize for EngineError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}

// -- Common languages --

pub fn common_languages() -> Vec<Language> {
    vec![
        Language { code: "auto".into(), label: "Auto".into() },
        Language { code: "fr".into(), label: "Français".into() },
        Language { code: "en".into(), label: "English".into() },
        Language { code: "es".into(), label: "Español".into() },
        Language { code: "de".into(), label: "Deutsch".into() },
    ]
}

// -- Catalog --

pub struct EngineCatalog {
    engines: Vec<Box<dyn ASREngine>>,
}

impl EngineCatalog {
    pub fn new() -> Self {
        let engines: Vec<Box<dyn ASREngine>> = vec![
            Box::new(whisper::WhisperEngine),
            Box::new(llm_local::LlmLocalEngine),
        ];

        Self { engines }
    }

    pub fn all_models(&self) -> Vec<ASRModel> {
        self.engines.iter().flat_map(|e| e.models()).collect()
    }

    pub fn engine_infos(&self) -> Vec<EngineInfo> {
        self.engines
            .iter()
            .map(|e| EngineInfo {
                id: e.engine_id().to_string(),
                name: e.display_name().to_string(),
                description: e.description().to_string(),
                category: e.category(),
                available: true,
                supported_language_codes: e.supported_languages().into_iter().map(|l| l.code).collect(),
            })
            .collect()
    }

    pub fn model_by_id(&self, id: &str) -> Option<ASRModel> {
        self.all_models().into_iter().find(|m| m.id == id)
    }

    pub fn downloaded_models(&self) -> Vec<ASRModel> {
        self.all_models().into_iter().filter(|m| m.is_downloaded()).collect()
    }

    pub fn recommended_model_ids(&self, language: &str) -> std::collections::HashSet<String> {
        self.engines.iter()
            .filter(|e| e.category() == EngineCategory::ASR)
            .filter_map(|e| e.recommended_model_id(language))
            .collect()
    }

    pub fn supported_languages(&self) -> Vec<Language> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for engine in &self.engines {
            for lang in engine.supported_languages() {
                if seen.insert(lang.code.clone()) {
                    result.push(lang);
                }
            }
        }
        result
    }
}
