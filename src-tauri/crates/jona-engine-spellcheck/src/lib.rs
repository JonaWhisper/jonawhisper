use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineCategory, EngineError,
    EngineRegistration, GpuMode, Language,
};
use std::any::Any;

pub struct SpellCheckEngine;

fn storage_dir() -> String {
    jona_types::models_dir()
        .join("spellcheck")
        .to_string_lossy()
        .to_string()
}

const GH_RAW: &str = "https://raw.githubusercontent.com/JonaWhisper/jonawhisper-spellcheck-dicts/main";

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
        vec![
            ASRModel {
                id: "spellcheck:fr".into(),
                engine_id: "spellcheck".into(),
                label: "Fran\u{00e7}ais".into(),
                filename: "fr".into(),
                url: String::new(),
                size: 9_840_000 + 90_000, // freq + bigrams
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "freq.txt".into(),
                            url: format!("{GH_RAW}/fr/freq.txt"),
                            size: 9_840_000,
                        },
                        DownloadFile {
                            filename: "bigram.txt".into(),
                            url: format!("{GH_RAW}/fr/bigram.txt"),
                            size: 90_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended_for: Some(vec!["fr".into()]),
                params: None,
                ram: Some(100_000_000), // ~100 MB in memory for 645K words
                lang_codes: Some(vec!["fr".into()]),
                runtime: None,
                quantization: None,
                ..Default::default()
            },
            ASRModel {
                id: "spellcheck:en".into(),
                engine_id: "spellcheck".into(),
                label: "English".into(),
                filename: "en".into(),
                url: String::new(),
                size: 1_335_000 + 5_140_000, // freq + bigrams
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "freq.txt".into(),
                            url: format!("{GH_RAW}/en/freq.txt"),
                            size: 1_335_000,
                        },
                        DownloadFile {
                            filename: "bigram.txt".into(),
                            url: format!("{GH_RAW}/en/bigram.txt"),
                            size: 5_140_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended_for: Some(vec!["en".into()]),
                params: None,
                ram: Some(30_000_000), // ~30 MB for 82K words
                lang_codes: Some(vec!["en".into()]),
                runtime: None,
                quantization: None,
                ..Default::default()
            },
        ]
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
        // No runtime context needed — symspell_correct.rs loads dicts directly
        Err(EngineError::LaunchFailed(
            "SpellCheck models are data-only, no context to create".into(),
        ))
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(SpellCheckEngine) }
}
