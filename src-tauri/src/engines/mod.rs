pub mod whisper;
pub mod canary;
pub mod parakeet;
pub mod qwen;
pub mod voxtral;
pub mod openai_api;
pub mod downloader;
pub mod llama;
pub mod bert;
pub mod pcs;
pub mod correction;
pub mod ort_session;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// -- Engine category --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineCategory {
    ASR,
    LLM,
    Punctuation,
    Correction,
}

// -- Download type --

/// Describes a single file within a multi-file download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadFile {
    pub filename: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DownloadType {
    #[default]
    SingleFile,
    /// Multiple files downloaded into a directory. Model `filename` is the directory name.
    /// Uses `download_marker` (e.g. ".complete") to track completion.
    MultiFile { files: Vec<DownloadFile> },
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
    /// Estimated RAM usage in bytes when loaded.
    pub ram: Option<u64>,
    /// Language codes this specific model excels at (None = inherits from engine).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang_codes: Option<Vec<String>>,
    /// Inference runtime for this model (e.g. "ort", "candle").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// Quantization format (e.g. "INT8", "Q5", "Q8", "FP32").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
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

// -- Cleanup dispatch --

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PunctRuntime { BertOrt, BertCandle, Pcs }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupKind {
    Correction,
    Punctuation(PunctRuntime),
    LocalLlm,
    CloudLlm(String),
    None,
}

impl CleanupKind {
    pub fn from_model_id(model_id: &str) -> Self {
        if model_id.is_empty() { return Self::None; }
        if let Some(pid) = model_id.strip_prefix("cloud:") { return Self::CloudLlm(pid.into()); }
        if model_id.starts_with("correction:") { return Self::Correction; }
        if model_id.starts_with("pcs-punctuation:") { return Self::Punctuation(PunctRuntime::Pcs); }
        if model_id.starts_with("bert-punctuation:") {
            let rt = EngineCatalog::new().model_by_id(model_id)
                .and_then(|m| m.runtime).unwrap_or_else(|| "ort".into());
            return match rt.as_str() {
                "candle" => Self::Punctuation(PunctRuntime::BertCandle),
                _ => Self::Punctuation(PunctRuntime::BertOrt),
            };
        }
        if model_id.starts_with("llama:") { return Self::LocalLlm; }
        Self::None
    }
}

/// Resolve a model ID from the catalog and verify it's downloaded.
pub fn resolve_model(model_id: &str) -> Result<(ASRModel, std::path::PathBuf), String> {
    let catalog = EngineCatalog::new();
    let model = catalog.model_by_id(model_id)
        .ok_or_else(|| format!("Model not found: {}", model_id))?;
    if !model.is_downloaded() {
        return Err(format!("Model not downloaded: {}", model_id));
    }
    let path = model.local_path();
    Ok((model, path))
}

// -- Catalog --

pub struct EngineCatalog {
    engines: Vec<Box<dyn ASREngine>>,
}

impl EngineCatalog {
    pub fn new() -> Self {
        let engines: Vec<Box<dyn ASREngine>> = vec![
            Box::new(whisper::WhisperEngine),
            Box::new(canary::CanaryEngine),
            Box::new(parakeet::ParakeetEngine),
            Box::new(qwen::QwenEngine),
            Box::new(voxtral::VoxtralEngine),
            Box::new(llama::LlamaEngine),
            Box::new(bert::BertPunctuationEngine),
            Box::new(pcs::PcsPunctuationEngine),
            Box::new(correction::CorrectionEngine),
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
