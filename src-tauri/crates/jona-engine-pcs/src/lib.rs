use jona_types::{ASREngine, ASRModel, DownloadType, EngineCategory, Language};

pub struct PcsPunctuationEngine;

fn storage_dir() -> String {
    jona_types::models_dir().join("pcs").to_string_lossy().to_string()
}

impl ASREngine for PcsPunctuationEngine {
    fn engine_id(&self) -> &str { "pcs-punctuation" }
    fn display_name(&self) -> &str { "PCS Punctuation" }
    fn category(&self) -> EngineCategory { EngineCategory::Punctuation }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "pcs-punctuation:47lang".into(),
                engine_id: "pcs-punctuation".into(),
                label: "Punct+Case 47 Languages".into(),
                filename: "punct_cap_seg_47lang.onnx".into(),
                url: "https://huggingface.co/1-800-BAD-CODE/punct_cap_seg_47_language/resolve/main/punct_cap_seg_47lang.onnx".into(),
                size: 232_900_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile,
                download_marker: None,
                recommended: false,
                params: Some(0.23),
                ram: Some(300_000_000),
                lang_codes: Some(vec![
                    "af".into(), "am".into(), "ar".into(), "bg".into(), "bn".into(),
                    "cs".into(), "da".into(), "de".into(), "el".into(), "en".into(),
                    "es".into(), "et".into(), "fa".into(), "fi".into(), "fr".into(),
                    "gu".into(), "hi".into(), "hr".into(), "hu".into(), "id".into(),
                    "is".into(), "it".into(), "ja".into(), "ka".into(), "kk".into(),
                    "km".into(), "kn".into(), "ko".into(), "lt".into(), "lv".into(),
                    "mk".into(), "ml".into(), "mn".into(), "mr".into(), "nl".into(),
                    "no".into(), "or".into(), "pl".into(), "pt".into(), "ro".into(),
                    "ru".into(), "sk".into(), "sl".into(), "ta".into(), "te".into(),
                    "tr".into(), "zh".into(),
                ]),
                quantization: Some("FP32".into()),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![]
    }

    fn description(&self) -> &str {
        "Punctuation, capitalization & segmentation for 47 languages. Smaller and more accurate than BERT."
    }
}
