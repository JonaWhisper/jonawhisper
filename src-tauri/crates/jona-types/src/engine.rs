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
    LanguageModel,
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

// -- Transcription result with optional per-word confidence --

/// Per-word confidence from ASR engine.
#[derive(Debug, Clone)]
pub struct WordConfidence {
    /// The word as it appears in the transcription text.
    pub word: String,
    /// Probability \[0.0, 1.0\]. None means confidence unknown.
    pub confidence: Option<f32>,
}

/// Result of ASR transcription, carrying optional per-word confidence scores.
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    /// Per-word confidences aligned to whitespace-delimited words in `text`.
    /// Empty vec = no confidence data (engine doesn't support it).
    pub word_confidences: Vec<WordConfidence>,
}

impl TranscriptionResult {
    /// Create a result with no confidence data.
    pub fn text_only(text: String) -> Self {
        Self { text, word_confidences: Vec::new() }
    }
}

/// Group sub-word tokens into word-level confidences.
/// Tokens starting with a space (BPE) or `\u{2581}` (SentencePiece) begin a new word.
/// For each word, confidence = minimum probability of its constituent tokens.
pub fn tokens_to_word_confidences(tokens: &[(String, f32)]) -> Vec<WordConfidence> {
    let mut result = Vec::new();
    let mut current_word = String::new();
    let mut min_prob: f32 = 1.0;

    for (text, prob) in tokens {
        let starts_new = text.starts_with(' ') || text.starts_with('\u{2581}');
        if starts_new && !current_word.is_empty() {
            result.push(WordConfidence {
                word: current_word.clone(),
                confidence: Some(min_prob),
            });
            current_word.clear();
            min_prob = 1.0;
        }
        let clean = text.trim_start_matches([' ', '\u{2581}']);
        current_word.push_str(clean);
        min_prob = min_prob.min(*prob);
    }

    if !current_word.is_empty() {
        result.push(WordConfidence {
            word: current_word,
            confidence: Some(min_prob),
        });
    }

    result
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
        -> Result<TranscriptionResult, EngineError>
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
