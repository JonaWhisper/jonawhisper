//! Core engine types, traits, and errors shared by all engine crates.

use serde::{Deserialize, Serialize};
use std::any::Any;
use std::path::{Path, PathBuf};

use crate::GpuMode;

// -- Engine auto-registration via inventory --

pub struct EngineRegistration {
    pub factory: fn() -> Box<dyn ASREngine>,
}

inventory::collect!(EngineRegistration);

// -- Engine category --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineCategory {
    ASR,
    LLM,
    Punctuation,
    Correction,
    SpellCheck,
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
    /// Languages this model is recommended for.
    /// None = not recommended, Some([]) = recommended for all, Some(["fr"]) = recommended for FR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_for: Option<Vec<String>>,
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
    /// SHA256 hash of the model file (for single-file models).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// SHA256 hashes per file (for multi-file models). Key = filename, value = hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_hashes: Option<std::collections::HashMap<String, String>>,
}

impl ASRModel {
    pub fn local_path(&self) -> PathBuf {
        let expanded = shellexpand::tilde(&self.storage_dir);
        PathBuf::from(expanded.as_ref()).join(&self.filename)
    }

    /// Whether this model is recommended for the given language.
    pub fn is_recommended_for(&self, language: &str) -> bool {
        self.recommended_for.as_ref().is_some_and(|langs| {
            langs.is_empty() || language == "auto" || langs.iter().any(|l| l == language)
        })
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
    fn recommended_model_id(&self, language: &str) -> Option<String> {
        self.models().into_iter()
            .find(|m| m.is_recommended_for(language))
            .map(|m| m.id)
    }

    // -- Inference methods (plug-and-play) --

    /// Cache key for context reuse. Override to include extra state (e.g. gpu_mode).
    fn context_key(&self, model: &ASRModel, _gpu_mode: GpuMode) -> String {
        model.id.clone()
    }

    /// Create an inference context for the given model.
    fn create_context(&self, _model: &ASRModel, _gpu_mode: GpuMode)
        -> Result<Box<dyn Any + Send>, EngineError>
    {
        Err(EngineError::LaunchFailed(format!("{}: no inference support", self.engine_id())))
    }

    /// Run ASR transcription using the given context.
    fn transcribe(&self, _ctx: &mut dyn Any, _audio_path: &Path, _language: &str)
        -> Result<String, EngineError>
    {
        Err(EngineError::LaunchFailed("Transcription not supported".into()))
    }

    /// Run text cleanup using the given context.
    fn cleanup(&self, _ctx: &mut dyn Any, _text: &str, _language: &str, _max_tokens: usize)
        -> Result<String, EngineError>
    {
        Err(EngineError::LaunchFailed("Cleanup not supported".into()))
    }

    /// Whether cleanup should run after finalize (punctuation, correction) vs before (LLM).
    /// Default: based on category.
    fn finalize_before_cleanup(&self) -> bool {
        matches!(self.category(), EngineCategory::LLM)
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
