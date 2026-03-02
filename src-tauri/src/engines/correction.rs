use super::*;

pub struct CorrectionEngine;

fn storage_dir() -> String {
    crate::state::models_dir()
        .join("correction")
        .to_string_lossy()
        .to_string()
}

impl ASREngine for CorrectionEngine {
    fn engine_id(&self) -> &str {
        "correction"
    }
    fn display_name(&self) -> &str {
        "T5 Correction"
    }
    fn category(&self) -> EngineCategory {
        EngineCategory::Correction
    }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "correction:gec-t5-small".into(),
                engine_id: "correction".into(),
                label: "GEC T5 Small".into(),
                filename: "gec-t5-small".into(),
                url: String::new(),
                size: 242_000_000 + 800_000 + 2_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "model.safetensors".into(),
                            url: "https://huggingface.co/Unbabel/gec-t5_small/resolve/main/model.safetensors".into(),
                            size: 242_000_000,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: "https://huggingface.co/Unbabel/gec-t5_small/resolve/main/config.json".into(),
                            size: 800_000,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: "https://huggingface.co/Unbabel/gec-t5_small/resolve/main/tokenizer.json".into(),
                            size: 2_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended: true,
                params: Some(0.06),
                ram: Some(350_000_000),
                lang_codes: Some(vec![
                    "en".into(),
                    "de".into(),
                    "fr".into(),
                    "es".into(),
                    "it".into(),
                    "pt".into(),
                    "nl".into(),
                    "ru".into(),
                    "zh".into(),
                    "ja".into(),
                    "ko".into(),
                ]),
                runtime: Some("candle".into()),
                ..Default::default()
            },
            ASRModel {
                id: "correction:t5-spell-fr".into(),
                engine_id: "correction".into(),
                label: "T5 Spell Correction FR".into(),
                filename: "t5-spell-fr".into(),
                url: String::new(),
                size: 892_000_000 + 1_400 + 2_400_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "model.safetensors".into(),
                            url: "https://huggingface.co/fdemelo/t5-base-spell-correction-fr/resolve/main/model.safetensors".into(),
                            size: 892_000_000,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: "https://huggingface.co/fdemelo/t5-base-spell-correction-fr/resolve/main/config.json".into(),
                            size: 1_400,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: "https://huggingface.co/fdemelo/t5-base-spell-correction-fr/resolve/main/tokenizer.json".into(),
                            size: 2_400_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended: false,
                params: Some(0.22),
                ram: Some(1_000_000_000),
                lang_codes: Some(vec!["fr".into()]),
                runtime: Some("candle".into()),
                ..Default::default()
            },
            ASRModel {
                id: "correction:flanec-large".into(),
                engine_id: "correction".into(),
                label: "FlanEC Large".into(),
                filename: "flanec-large".into(),
                url: String::new(),
                size: 990_000_000 + 1_500 + 2_400_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "model.safetensors".into(),
                            url: "https://huggingface.co/morenolq/flanec-large-cd/resolve/main/model.safetensors".into(),
                            size: 990_000_000,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: "https://huggingface.co/morenolq/flanec-large-cd/resolve/main/config.json".into(),
                            size: 1_500,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: "https://huggingface.co/morenolq/flanec-large-cd/resolve/main/tokenizer.json".into(),
                            size: 2_400_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended: false,
                params: Some(0.25),
                ram: Some(1_200_000_000),
                lang_codes: Some(vec!["en".into()]),
                runtime: Some("candle".into()),
                ..Default::default()
            },
            ASRModel {
                id: "correction:flan-t5-grammar".into(),
                engine_id: "correction".into(),
                label: "Flan-T5 Large Grammar".into(),
                filename: "flan-t5-grammar".into(),
                url: String::new(),
                size: 3_130_000_000 + 1_500 + 2_400_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "model.safetensors".into(),
                            url: "https://huggingface.co/pszemraj/flan-t5-large-grammar-synthesis/resolve/main/model.safetensors".into(),
                            size: 3_130_000_000,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: "https://huggingface.co/pszemraj/flan-t5-large-grammar-synthesis/resolve/main/config.json".into(),
                            size: 1_500,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: "https://huggingface.co/pszemraj/flan-t5-large-grammar-synthesis/resolve/main/tokenizer.json".into(),
                            size: 2_400_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended: false,
                params: Some(0.78),
                ram: Some(3_500_000_000),
                lang_codes: Some(vec!["en".into()]),
                runtime: Some("candle".into()),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![]
    }

    fn description(&self) -> &str {
        "T5 models for post-ASR text correction: grammar, spelling, and punctuation."
    }
}
