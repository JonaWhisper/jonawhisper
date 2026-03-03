use super::*;

pub struct VoxtralEngine;

fn storage_dir() -> String {
    crate::state::models_dir().join("voxtral").to_string_lossy().to_string()
}

const HF_BASE: &str = "https://huggingface.co/mistralai/Voxtral-Mini-4B-Realtime-2602/resolve/main/";

impl ASREngine for VoxtralEngine {
    fn engine_id(&self) -> &str { "voxtral" }
    fn display_name(&self) -> &str { "Voxtral" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "voxtral:mini-4b-realtime".into(),
                engine_id: "voxtral".into(),
                label: "Voxtral Realtime 4B".into(),
                filename: "mini-4b-realtime".into(),
                url: String::new(),
                size: 8_859_462_744 + 14_910_348 + 1_343,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "consolidated.safetensors".into(),
                            url: format!("{}consolidated.safetensors", HF_BASE),
                            size: 8_859_462_744,
                        },
                        DownloadFile {
                            filename: "tekken.json".into(),
                            url: format!("{}tekken.json", HF_BASE),
                            size: 14_910_348,
                        },
                        DownloadFile {
                            filename: "params.json".into(),
                            url: format!("{}params.json", HF_BASE),
                            size: 1_343,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                wer: None,
                rtf: None,
                recommended: false,
                params: Some(4.4),
                ram: Some(10_000_000_000),
                lang_codes: Some(vec![
                    "en".into(), "fr".into(), "de".into(), "es".into(), "it".into(),
                    "pt".into(), "nl".into(), "ru".into(), "pl".into(), "tr".into(),
                    "ja".into(), "ko".into(), "zh".into(),
                ]),
                runtime: Some("metal".into()),
                quantization: Some("BF16".into()),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "en".into(), label: "English".into() },
            Language { code: "fr".into(), label: "Français".into() },
            Language { code: "de".into(), label: "Deutsch".into() },
            Language { code: "es".into(), label: "Español".into() },
            Language { code: "it".into(), label: "Italiano".into() },
            Language { code: "pt".into(), label: "Português".into() },
            Language { code: "nl".into(), label: "Nederlands".into() },
            Language { code: "ru".into(), label: "Русский".into() },
            Language { code: "pl".into(), label: "Polski".into() },
            Language { code: "tr".into(), label: "Türkçe".into() },
            Language { code: "ja".into(), label: "日本語".into() },
            Language { code: "ko".into(), label: "한국어".into() },
            Language { code: "zh".into(), label: "中文".into() },
        ]
    }

    fn description(&self) -> &str {
        "Mistral Voxtral Realtime 4B. 13 languages, Metal GPU acceleration."
    }
}
