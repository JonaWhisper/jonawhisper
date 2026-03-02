use super::*;

pub struct BertPunctuationEngine;

fn storage_dir() -> String {
    crate::state::models_dir().join("bert").to_string_lossy().to_string()
}

impl ASREngine for BertPunctuationEngine {
    fn engine_id(&self) -> &str { "bert-punctuation" }
    fn display_name(&self) -> &str { "BERT Punctuation" }
    fn category(&self) -> EngineCategory { EngineCategory::Punctuation }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "bert-punctuation:fullstop-multilang-large".into(),
                engine_id: "bert-punctuation".into(),
                label: "Fullstop Multilang Large (INT8)".into(),
                filename: "model_quantized.onnx".into(),
                url: "https://huggingface.co/ldenoue/fullstop-punctuation-multilang-large/resolve/main/onnx/model_quantized.onnx".into(),
                size: 562_000_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile,
                download_marker: None,
                recommended: true,
                params: Some(0.56),
                ram: Some(600_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "de".into(), "it".into()]),
                ..Default::default()
            },
            ASRModel {
                id: "bert-punctuation:fullstop-multilingual-base".into(),
                engine_id: "bert-punctuation".into(),
                label: "Fullstop Multilingual Base (FP32)".into(),
                filename: "model.safetensors".into(),
                url: "https://huggingface.co/oliverguhr/fullstop-punctuation-multilingual-base/resolve/main/model.safetensors".into(),
                size: 1_112_000_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile,
                download_marker: None,
                recommended: false,
                params: Some(0.28),
                ram: Some(560_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "de".into(), "it".into(), "nl".into()]),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        // Don't pollute the global language selector — it's for ASR, not punctuation.
        // BERT language support is indicated via lang_codes on each model.
        vec![]
    }

    fn description(&self) -> &str {
        "BERT-based punctuation restoration. Fast (~100ms), adds periods, commas, question marks."
    }
}
