pub mod downloader;
pub mod ort_session;
pub mod mel;
pub mod audio;
pub mod common;
pub mod llm_prompt;

// Re-export engine types from jona-types for backward compatibility
pub use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineCategory,
    EngineError, EngineInfo, Language, common_languages,
};

use std::sync::OnceLock;

use jona_types::EngineRegistration;

/// Global engine catalog singleton — initialized by the app at startup.
static CATALOG: OnceLock<EngineCatalog> = OnceLock::new();

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

    /// Initialize the global catalog with all engine crates.
    /// Must be called once at app startup. Returns `false` if already initialized.
    pub fn init(engines: Vec<Box<dyn ASREngine>>) -> bool {
        CATALOG.set(Self::build(engines)).is_ok()
    }

    /// Initialize from inventory auto-registration (no manual engine list needed).
    /// Each engine crate submits an `EngineRegistration` via `inventory::submit!`.
    pub fn init_auto() -> bool {
        let engines: Vec<Box<dyn ASREngine>> = inventory::iter::<EngineRegistration>()
            .map(|reg| (reg.factory)())
            .collect();
        log::info!("EngineCatalog: auto-registered {} engines", engines.len());
        Self::init(engines)
    }

    fn build(engines: Vec<Box<dyn ASREngine>>) -> Self {
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

    /// Look up an engine by its ID. Used for dynamic dispatch.
    pub fn engine_by_id(&self, id: &str) -> Option<&dyn ASREngine> {
        self.engines.iter()
            .find(|e| e.engine_id() == id)
            .map(|e| &**e)
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
