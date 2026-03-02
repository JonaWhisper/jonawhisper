use super::*;

pub struct PcsPunctuationEngine;

fn storage_dir() -> String {
    crate::state::models_dir().join("pcs").to_string_lossy().to_string()
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
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "af".into(), label: "Afrikaans".into() },
            Language { code: "am".into(), label: "Amharic".into() },
            Language { code: "ar".into(), label: "Arabic".into() },
            Language { code: "bg".into(), label: "Bulgarian".into() },
            Language { code: "bn".into(), label: "Bengali".into() },
            Language { code: "cs".into(), label: "Czech".into() },
            Language { code: "da".into(), label: "Danish".into() },
            Language { code: "de".into(), label: "Deutsch".into() },
            Language { code: "el".into(), label: "Greek".into() },
            Language { code: "en".into(), label: "English".into() },
            Language { code: "es".into(), label: "Español".into() },
            Language { code: "et".into(), label: "Estonian".into() },
            Language { code: "fa".into(), label: "Persian".into() },
            Language { code: "fi".into(), label: "Finnish".into() },
            Language { code: "fr".into(), label: "Français".into() },
            Language { code: "gu".into(), label: "Gujarati".into() },
            Language { code: "hi".into(), label: "Hindi".into() },
            Language { code: "hr".into(), label: "Croatian".into() },
            Language { code: "hu".into(), label: "Hungarian".into() },
            Language { code: "id".into(), label: "Indonesian".into() },
            Language { code: "is".into(), label: "Icelandic".into() },
            Language { code: "it".into(), label: "Italiano".into() },
            Language { code: "ja".into(), label: "Japanese".into() },
            Language { code: "ka".into(), label: "Georgian".into() },
            Language { code: "kk".into(), label: "Kazakh".into() },
            Language { code: "km".into(), label: "Khmer".into() },
            Language { code: "kn".into(), label: "Kannada".into() },
            Language { code: "ko".into(), label: "Korean".into() },
            Language { code: "lt".into(), label: "Lithuanian".into() },
            Language { code: "lv".into(), label: "Latvian".into() },
            Language { code: "mk".into(), label: "Macedonian".into() },
            Language { code: "ml".into(), label: "Malayalam".into() },
            Language { code: "mn".into(), label: "Mongolian".into() },
            Language { code: "mr".into(), label: "Marathi".into() },
            Language { code: "nl".into(), label: "Dutch".into() },
            Language { code: "no".into(), label: "Norwegian".into() },
            Language { code: "or".into(), label: "Oriya".into() },
            Language { code: "pl".into(), label: "Polish".into() },
            Language { code: "pt".into(), label: "Portuguese".into() },
            Language { code: "ro".into(), label: "Romanian".into() },
            Language { code: "ru".into(), label: "Russian".into() },
            Language { code: "sk".into(), label: "Slovak".into() },
            Language { code: "sl".into(), label: "Slovenian".into() },
            Language { code: "ta".into(), label: "Tamil".into() },
            Language { code: "te".into(), label: "Telugu".into() },
            Language { code: "tr".into(), label: "Turkish".into() },
            Language { code: "zh".into(), label: "Chinese".into() },
        ]
    }

    fn description(&self) -> &str {
        "Punctuation, capitalization & segmentation for 47 languages. Smaller and more accurate than BERT."
    }
}
