pub mod voxtral;
pub mod downloader;
pub mod llama;
pub mod bert;
pub mod pcs;
pub mod correction;
pub mod ort_session;
pub mod mel;
pub mod audio;

// Re-export engine types from jona-types for backward compatibility
pub use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineCategory,
    EngineError, EngineInfo, Language, common_languages,
};

use std::sync::OnceLock;

/// Global engine catalog singleton — initialized by the app at startup.
static CATALOG: OnceLock<EngineCatalog> = OnceLock::new();

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
            let rt = EngineCatalog::global().model_by_id(model_id)
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
    let catalog = EngineCatalog::global();
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
    /// Access the global singleton catalog.
    /// Panics if `init()` has not been called.
    pub fn global() -> &'static Self {
        CATALOG.get().expect("EngineCatalog not initialized — call EngineCatalog::init() at startup")
    }

    /// Initialize the global catalog with external engine crates + internal engines.
    /// Must be called once at app startup. Returns `false` if already initialized.
    pub fn init(extra: Vec<Box<dyn ASREngine>>) -> bool {
        CATALOG.set(Self::build(extra)).is_ok()
    }

    fn build(extra: Vec<Box<dyn ASREngine>>) -> Self {
        let mut engines: Vec<Box<dyn ASREngine>> = extra;
        // Internal engines (not yet extracted into their own crates)
        engines.extend([
            Box::new(voxtral::VoxtralEngine) as Box<dyn ASREngine>,
            Box::new(llama::LlamaEngine),
            Box::new(bert::BertPunctuationEngine),
            Box::new(pcs::PcsPunctuationEngine),
            Box::new(correction::CorrectionEngine),
        ]);
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
        self.engines.iter()
            .find_map(|e| e.models().into_iter().find(|m| m.id == id))
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
