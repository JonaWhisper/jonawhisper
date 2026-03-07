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

/// GitHub Releases URL — `/releases/latest/download/` auto-redirects to the newest release.
const GH_RELEASE: &str =
    "https://github.com/JonaWhisper/jonawhisper-spellcheck-dicts/releases/latest/download";

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
                size: 9_840_000 + 90_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "freq.txt".into(),
                            url: format!("{GH_RELEASE}/fr-freq.txt"),
                            size: 9_840_000,
                        },
                        DownloadFile {
                            filename: "bigram.txt".into(),
                            url: format!("{GH_RELEASE}/fr-bigram.txt"),
                            size: 90_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended_for: Some(vec!["fr".into()]),
                params: None,
                ram: Some(100_000_000),
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
                size: 1_335_000 + 5_140_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "freq.txt".into(),
                            url: format!("{GH_RELEASE}/en-freq.txt"),
                            size: 1_335_000,
                        },
                        DownloadFile {
                            filename: "bigram.txt".into(),
                            url: format!("{GH_RELEASE}/en-bigram.txt"),
                            size: 5_140_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended_for: Some(vec!["en".into()]),
                params: None,
                ram: Some(30_000_000),
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
        Err(EngineError::LaunchFailed(
            "SpellCheck models are data-only, no context to create".into(),
        ))
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(SpellCheckEngine) }
}
