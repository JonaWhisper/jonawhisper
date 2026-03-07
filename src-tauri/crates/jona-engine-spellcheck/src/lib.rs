use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineCategory, EngineError,
    EngineRegistration, GpuMode, Language,
};
use serde::Deserialize;
use std::any::Any;
use std::collections::HashMap;

pub struct SpellCheckEngine;

fn storage_dir() -> String {
    jona_types::models_dir()
        .join("spellcheck")
        .to_string_lossy()
        .to_string()
}

/// GitHub Releases URL — `/releases/latest/download/` auto-redirects to the newest release.
const GH_RELEASE: &str =
    "https://github.com/JonaWhisper/jonawhisper-spellcheck-dicts/releases/latest/download";

/// Manifest embedded at build time (fallback when no cached version on disk).
const EMBEDDED_MANIFEST: &str = include_str!("../manifest.json");

// -- Manifest deserialization --

#[derive(Deserialize)]
struct Manifest {
    languages: HashMap<String, ManifestLang>,
}

#[derive(Deserialize)]
struct ManifestLang {
    label: Option<String>,
    ram: Option<u64>,
    files: Option<HashMap<String, ManifestFile>>,
    // Legacy format: files at top level (no "files" wrapper)
    #[serde(flatten)]
    legacy_files: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct ManifestFile {
    size: u64,
}

impl ManifestLang {
    fn file_size(&self, name: &str) -> u64 {
        // New format: files.{name}.size
        if let Some(files) = &self.files {
            if let Some(f) = files.get(name) {
                return f.size;
            }
        }
        // Legacy format: {name}.size at top level
        if let Some(val) = self.legacy_files.get(name) {
            if let Some(size) = val.get("size").and_then(|s| s.as_u64()) {
                return size;
            }
        }
        0
    }

}

/// Build an ASRModel from manifest data for a single language.
fn model_from_manifest(code: &str, lang: &ManifestLang) -> ASRModel {
    let label = lang
        .label
        .clone()
        .unwrap_or_else(|| code.to_string());
    let freq_size = lang.file_size("freq.txt");
    let bigram_size = lang.file_size("bigram.txt");
    // RAM estimate: ~10x freq file size (SymSpell BK-tree + delete hashes)
    let ram = lang.ram.unwrap_or(freq_size * 10);

    ASRModel {
        id: format!("spellcheck:{code}"),
        engine_id: "spellcheck".into(),
        label,
        filename: code.into(),
        url: String::new(),
        size: freq_size + bigram_size,
        storage_dir: storage_dir(),
        download_type: DownloadType::MultiFile {
            files: vec![
                DownloadFile {
                    filename: "freq.txt".into(),
                    url: format!("{GH_RELEASE}/{code}-freq.txt"),
                    size: freq_size,
                },
                DownloadFile {
                    filename: "bigram.txt".into(),
                    url: format!("{GH_RELEASE}/{code}-bigram.txt"),
                    size: bigram_size,
                },
            ],
        },
        download_marker: Some(".complete".into()),
        recommended_for: Some(vec![code.into()]),
        params: None,
        ram: Some(ram),
        lang_codes: Some(vec![code.into()]),
        runtime: None,
        quantization: None,
        ..Default::default()
    }
}

/// Load manifest from disk cache (runtime updates) or fall back to embedded.
fn load_manifest() -> Vec<ASRModel> {
    let cached = jona_types::models_dir()
        .join("spellcheck")
        .join("manifest.json");

    let json = if cached.exists() {
        match std::fs::read_to_string(&cached) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to read cached manifest: {}, using embedded", e);
                EMBEDDED_MANIFEST.to_string()
            }
        }
    } else {
        EMBEDDED_MANIFEST.to_string()
    };

    parse_manifest(&json)
}

fn parse_manifest(json: &str) -> Vec<ASRModel> {
    let manifest: Manifest = match serde_json::from_str(json) {
        Ok(m) => m,
        Err(e) => {
            log::error!("Failed to parse spellcheck manifest: {}", e);
            return Vec::new();
        }
    };

    let mut models: Vec<ASRModel> = manifest
        .languages
        .iter()
        .map(|(code, lang)| model_from_manifest(code, lang))
        .collect();

    // Sort: base languages first, then regional variants
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models
}

impl ASREngine for SpellCheckEngine {
    fn engine_id(&self) -> &str {
        "spellcheck"
    }
    fn display_name(&self) -> &str {
        "SymSpell Dictionaries"
    }
    fn category(&self) -> EngineCategory {
        EngineCategory::SpellCheck
    }

    fn models(&self) -> Vec<ASRModel> {
        load_manifest()
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![]
    }

    fn description(&self) -> &str {
        "SymSpell frequency dictionaries for spell-checking. Download per language."
    }

    fn create_context(
        &self,
        _model: &ASRModel,
        _gpu_mode: GpuMode,
    ) -> Result<Box<dyn Any + Send>, EngineError> {
        Err(EngineError::LaunchFailed(
            "SpellCheck models are data-only, no context to create".into(),
        ))
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(SpellCheckEngine) }
}
