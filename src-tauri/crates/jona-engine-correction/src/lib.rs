mod inference;

use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineCategory, EngineError,
    EngineRegistration, GpuMode, Language,
};
use std::any::Any;

pub use inference::{T5Context, correct};

pub struct CorrectionEngine;

fn storage_dir() -> String {
    jona_types::models_dir().join("correction").to_string_lossy().to_string()
}

const HF_BASE: &str = "https://huggingface.co/JonaWhisper";

impl ASREngine for CorrectionEngine {
    fn engine_id(&self) -> &str { "correction" }
    fn display_name(&self) -> &str { "T5 Correction" }
    fn category(&self) -> EngineCategory { EngineCategory::Correction }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "correction:gec-t5-small".into(),
                engine_id: "correction".into(),
                label: "GEC T5 Small".into(),
                filename: "gec-t5-small".into(),
                url: String::new(),
                size: 376_325_263,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder_model.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-gec-t5-small-onnx/resolve/main/encoder_model.onnx"),
                            size: 141_410_256,
                        },
                        DownloadFile {
                            filename: "decoder_model.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-gec-t5-small-onnx/resolve/main/decoder_model.onnx"),
                            size: 232_491_073,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-gec-t5-small-onnx/resolve/main/config.json"),
                            size: 1_500,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-gec-t5-small-onnx/resolve/main/tokenizer.json"),
                            size: 2_422_434,
                        },
                    ],
                },
                download_marker: Some(".complete_v2".into()),
                recommended_for: Some(vec![]),
                params: Some(0.06),
                ram: Some(500_000_000),
                lang_codes: Some(vec![
                    "en".into(), "de".into(), "fr".into(), "es".into(), "it".into(),
                    "pt".into(), "nl".into(), "ru".into(), "zh".into(), "ja".into(), "ko".into(),
                ]),
                runtime: Some("ort".into()),
                quantization: Some("FP32".into()),
                ..Default::default()
            },
            ASRModel {
                id: "correction:t5-spell-fr".into(),
                engine_id: "correction".into(),
                label: "T5 Spell Correction FR".into(),
                filename: "t5-spell-fr".into(),
                url: String::new(),
                size: 1_091_683_357,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder_model.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-t5-spell-fr-onnx/resolve/main/encoder_model.onnx"),
                            size: 438_588_825,
                        },
                        DownloadFile {
                            filename: "decoder_model.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-t5-spell-fr-onnx/resolve/main/decoder_model.onnx"),
                            size: 650_668_829,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-t5-spell-fr-onnx/resolve/main/config.json"),
                            size: 1_468,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-t5-spell-fr-onnx/resolve/main/tokenizer.json"),
                            size: 2_424_235,
                        },
                    ],
                },
                download_marker: Some(".complete_v2".into()),
                recommended_for: Some(vec!["fr".into()]),
                params: Some(0.22),
                ram: Some(1_200_000_000),
                lang_codes: Some(vec!["fr".into()]),
                runtime: Some("ort".into()),
                quantization: Some("FP32".into()),
                ..Default::default()
            },
            ASRModel {
                id: "correction:flanec-large".into(),
                engine_id: "correction".into(),
                label: "FlanEC Large".into(),
                filename: "flanec-large".into(),
                url: String::new(),
                size: 3_267_721_038,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder_model.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-large-onnx/resolve/main/encoder_model.onnx"),
                            size: 1_365_296_186,
                        },
                        DownloadFile {
                            filename: "decoder_model.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-large-onnx/resolve/main/decoder_model.onnx"),
                            size: 1_900_001_851,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-large-onnx/resolve/main/config.json"),
                            size: 767,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-large-onnx/resolve/main/tokenizer.json"),
                            size: 2_422_234,
                        },
                    ],
                },
                download_marker: Some(".complete_v2".into()),
                recommended_for: None,
                params: Some(0.80),
                ram: Some(3_500_000_000),
                lang_codes: Some(vec!["en".into()]),
                runtime: Some("ort".into()),
                quantization: Some("FP32".into()),
                ..Default::default()
            },
            ASRModel {
                id: "correction:flanec-base".into(),
                engine_id: "correction".into(),
                label: "FlanEC Base".into(),
                filename: "flanec-base".into(),
                url: String::new(),
                size: 1_092_005_311,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder_model.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-base-onnx/resolve/main/encoder_model.onnx"),
                            size: 438_705_681,
                        },
                        DownloadFile {
                            filename: "decoder_model.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-base-onnx/resolve/main/decoder_model.onnx"),
                            size: 650_875_887,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-base-onnx/resolve/main/config.json"),
                            size: 1_509,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-base-onnx/resolve/main/tokenizer.json"),
                            size: 2_422_234,
                        },
                    ],
                },
                download_marker: Some(".complete_v2".into()),
                recommended_for: Some(vec!["en".into()]),
                params: Some(0.25),
                ram: Some(1_200_000_000),
                lang_codes: Some(vec!["en".into()]),
                runtime: Some("ort".into()),
                quantization: Some("FP32".into()),
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

    fn create_context(&self, model: &ASRModel, _gpu_mode: GpuMode)
        -> Result<Box<dyn Any + Send>, EngineError>
    {
        let ctx = T5Context::load(&model.local_path())
            .map_err(EngineError::LaunchFailed)?;
        Ok(Box::new(ctx))
    }

    fn cleanup(&self, ctx: &mut dyn Any, text: &str, _language: &str, _max_tokens: usize)
        -> Result<String, EngineError>
    {
        let ctx = ctx.downcast_mut::<T5Context>()
            .ok_or_else(|| EngineError::LaunchFailed("Invalid T5 context".into()))?;
        correct(ctx, text).map_err(EngineError::LaunchFailed)
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(CorrectionEngine) }
}
