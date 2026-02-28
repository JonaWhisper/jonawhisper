use super::*;

pub struct BertPunctuationEngine;

const BERT_STORAGE_DIR: &str = "~/.local/share/whisper-dictate/bert";

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
                storage_dir: BERT_STORAGE_DIR.into(),
                download_type: DownloadType::SingleFile,
                download_marker: None,
                recommended: true,
                params: Some(0.56),
                ram: Some(600_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "de".into(), "it".into()]),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "fr".into(), label: "Français".into() },
            Language { code: "en".into(), label: "English".into() },
            Language { code: "de".into(), label: "Deutsch".into() },
            Language { code: "it".into(), label: "Italiano".into() },
        ]
    }

    fn description(&self) -> &str {
        "BERT-based punctuation restoration. Fast (~100ms), adds periods, commas, question marks."
    }
}
