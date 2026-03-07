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

fn spellcheck_model(
    code: &str,
    label: &str,
    freq_size: u64,
    bigram_size: u64,
    ram: u64,
) -> ASRModel {
    ASRModel {
        id: format!("spellcheck:{code}"),
        engine_id: "spellcheck".into(),
        label: label.into(),
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
            spellcheck_model("fr", "Fran\u{00e7}ais", 9_840_000, 90_000, 100_000_000),
            spellcheck_model("fr-be", "Fran\u{00e7}ais (Belgique)", 9_900_000, 90_000, 100_000_000),
            spellcheck_model("fr-ca", "Fran\u{00e7}ais (Qu\u{00e9}bec)", 9_900_000, 90_000, 100_000_000),
            spellcheck_model("fr-ch", "Fran\u{00e7}ais (Suisse)", 9_900_000, 90_000, 100_000_000),
            spellcheck_model("en", "English", 1_335_000, 5_140_000, 30_000_000),
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
