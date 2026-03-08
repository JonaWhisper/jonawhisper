use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineCategory, EngineError,
    EngineRegistration, GpuMode, Language,
};
use serde::Deserialize;
use std::any::Any;
use std::collections::HashMap;

pub struct SpellCheckEngine;

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
        storage_dir: jona_types::engine_storage_dir("spellcheck"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use jona_types::{ASREngine, DownloadType};

    #[test]
    fn engine_registers_as_spellcheck() {
        let engine = SpellCheckEngine;
        assert_eq!(engine.engine_id(), "spellcheck");
        assert_eq!(engine.category(), jona_types::EngineCategory::SpellCheck);
    }

    #[test]
    fn spellcheck_does_not_pollute_language_selector() {
        let engine = SpellCheckEngine;
        assert!(engine.supported_languages().is_empty());
    }

    #[test]
    fn manifest_provides_downloadable_dictionaries() {
        // The embedded manifest should produce at least one dictionary model
        // that the user can download for spellchecking.
        let models = parse_manifest(EMBEDDED_MANIFEST);
        assert!(!models.is_empty(), "User must have at least one spellcheck dictionary to download");
    }

    #[test]
    fn no_duplicate_dictionaries() {
        let engine = SpellCheckEngine;
        let models = engine.models();
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count, "Duplicate dictionaries would confuse the user");
    }

    #[test]
    fn all_download_urls_are_secure() {
        let engine = SpellCheckEngine;
        for model in engine.models() {
            if let DownloadType::MultiFile { files } = &model.download_type {
                for file in files {
                    assert!(file.url.starts_with("https://"),
                        "Dict {} file {} has insecure download URL: {}", model.id, file.filename, file.url);
                }
            }
        }
    }

    #[test]
    fn dictionaries_include_french_and_english() {
        // The two primary languages of the app must have dictionaries available.
        let engine = SpellCheckEngine;
        let models = engine.models();
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.iter().any(|id| id.contains("fr")),
            "French spellcheck dictionary must be available");
        assert!(ids.iter().any(|id| id.contains("en")),
            "English spellcheck dictionary must be available");
    }
}
