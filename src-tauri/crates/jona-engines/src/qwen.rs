use super::*;

pub struct QwenEngine;

fn storage_dir() -> String {
    jona_types::models_dir().join("qwen-asr").to_string_lossy().to_string()
}

impl ASREngine for QwenEngine {
    fn engine_id(&self) -> &str { "qwen-asr" }
    fn display_name(&self) -> &str { "Qwen3-ASR" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "qwen-asr:0.6b".into(),
                engine_id: "qwen-asr".into(),
                label: "Qwen3 ASR".into(),
                filename: "0.6b".into(),
                url: String::new(),
                size: 1_880_000_000 + 2_780_000 + 1_670_000, // safetensors + vocab + merges
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "model.safetensors".into(),
                            url: "https://huggingface.co/Qwen/Qwen3-ASR-0.6B/resolve/main/model.safetensors".into(),
                            size: 1_880_000_000,
                        },
                        DownloadFile {
                            filename: "vocab.json".into(),
                            url: "https://huggingface.co/Qwen/Qwen3-ASR-0.6B/resolve/main/vocab.json".into(),
                            size: 2_780_000,
                        },
                        DownloadFile {
                            filename: "merges.txt".into(),
                            url: "https://huggingface.co/Qwen/Qwen3-ASR-0.6B/resolve/main/merges.txt".into(),
                            size: 1_670_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                wer: Some(2.0),
                rtf: Some(0.15),
                recommended: false,
                params: Some(0.6),
                ram: Some(2_000_000_000),
                lang_codes: Some(vec![
                    "en".into(), "fr".into(), "zh".into(), "ja".into(), "ko".into(),
                    "de".into(), "es".into(), "pt".into(), "it".into(), "ru".into(),
                    "ar".into(), "tr".into(), "hi".into(), "th".into(), "vi".into(),
                    "id".into(), "ms".into(), "nl".into(), "sv".into(), "da".into(),
                    "fi".into(), "pl".into(), "cs".into(), "ro".into(), "hu".into(),
                    "el".into(), "fa".into(), "fil".into(), "mk".into(),
                ]),
                runtime: Some("accelerate".into()),
                quantization: Some("BF16".into()),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "en".into(), label: "English".into() },
            Language { code: "fr".into(), label: "Français".into() },
            Language { code: "zh".into(), label: "中文".into() },
            Language { code: "ja".into(), label: "日本語".into() },
            Language { code: "ko".into(), label: "한국어".into() },
            Language { code: "de".into(), label: "Deutsch".into() },
            Language { code: "es".into(), label: "Español".into() },
            Language { code: "pt".into(), label: "Português".into() },
            Language { code: "it".into(), label: "Italiano".into() },
            Language { code: "ru".into(), label: "Русский".into() },
            Language { code: "ar".into(), label: "العربية".into() },
            Language { code: "tr".into(), label: "Türkçe".into() },
            Language { code: "hi".into(), label: "हिन्दी".into() },
            Language { code: "th".into(), label: "ไทย".into() },
            Language { code: "vi".into(), label: "Tiếng Việt".into() },
            Language { code: "id".into(), label: "Bahasa Indonesia".into() },
            Language { code: "ms".into(), label: "Bahasa Melayu".into() },
            Language { code: "nl".into(), label: "Nederlands".into() },
            Language { code: "sv".into(), label: "Svenska".into() },
            Language { code: "da".into(), label: "Dansk".into() },
            Language { code: "fi".into(), label: "Suomi".into() },
            Language { code: "pl".into(), label: "Polski".into() },
            Language { code: "cs".into(), label: "Čeština".into() },
            Language { code: "ro".into(), label: "Română".into() },
            Language { code: "hu".into(), label: "Magyar".into() },
            Language { code: "el".into(), label: "Ελληνικά".into() },
            Language { code: "fa".into(), label: "فارسی".into() },
            Language { code: "fil".into(), label: "Filipino".into() },
            Language { code: "mk".into(), label: "Македонски".into() },
        ]
    }

    fn description(&self) -> &str {
        "Alibaba Qwen3-ASR encoder-decoder LLM. 30 languages, Apple Accelerate (AMX) acceleration."
    }
}
