mod inference;

use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineCategory, EngineError,
    EngineRegistration, GpuMode, Language,
};
use std::any::Any;

pub use inference::{T5Context, correct};

pub struct CorrectionEngine;

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
                size: 96_391_294,
                storage_dir: jona_types::engine_storage_dir("correction"),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder_model_int8.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-gec-t5-small-onnx/resolve/main/encoder_model_int8.onnx"),
                            size: 35_518_119,
                        },
                        DownloadFile {
                            filename: "decoder_model_int8.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-gec-t5-small-onnx/resolve/main/decoder_model_int8.onnx"),
                            size: 58_449_240,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-gec-t5-small-onnx/resolve/main/config.json"),
                            size: 1_501,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-gec-t5-small-onnx/resolve/main/tokenizer.json"),
                            size: 2_422_434,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended_for: Some(vec![]),
                params: Some(0.06),
                ram: Some(200_000_000),
                lang_codes: Some(vec![
                    "en".into(), "de".into(), "fr".into(), "es".into(), "it".into(),
                    "pt".into(), "nl".into(), "ru".into(), "zh".into(), "ja".into(), "ko".into(),
                ]),
                runtime: Some("ort".into()),
                quantization: Some("INT8".into()),
                ..Default::default()
            },
            ASRModel {
                id: "correction:t5-spell-fr".into(),
                engine_id: "correction".into(),
                label: "T5 Spell Correction FR".into(),
                filename: "t5-spell-fr".into(),
                url: String::new(),
                size: 275_706_583,
                storage_dir: jona_types::engine_storage_dir("correction"),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder_model_int8.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-t5-spell-fr-onnx/resolve/main/encoder_model_int8.onnx"),
                            size: 109_979_870,
                        },
                        DownloadFile {
                            filename: "decoder_model_int8.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-t5-spell-fr-onnx/resolve/main/decoder_model_int8.onnx"),
                            size: 163_300_989,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-t5-spell-fr-onnx/resolve/main/config.json"),
                            size: 1_469,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-t5-spell-fr-onnx/resolve/main/tokenizer.json"),
                            size: 2_424_255,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended_for: Some(vec!["fr".into()]),
                params: Some(0.22),
                ram: Some(400_000_000),
                lang_codes: Some(vec!["fr".into()]),
                runtime: Some("ort".into()),
                quantization: Some("INT8".into()),
                ..Default::default()
            },
            ASRModel {
                id: "correction:flanec-large".into(),
                engine_id: "correction".into(),
                label: "FlanEC Large".into(),
                filename: "flanec-large".into(),
                url: String::new(),
                size: 820_920_089,
                storage_dir: jona_types::engine_storage_dir("correction"),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder_model_int8.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-large-onnx/resolve/main/encoder_model_int8.onnx"),
                            size: 342_107_652,
                        },
                        DownloadFile {
                            filename: "decoder_model_int8.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-large-onnx/resolve/main/decoder_model_int8.onnx"),
                            size: 476_389_435,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-large-onnx/resolve/main/config.json"),
                            size: 768,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-large-onnx/resolve/main/tokenizer.json"),
                            size: 2_422_234,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended_for: None,
                params: Some(0.80),
                ram: Some(1_200_000_000),
                lang_codes: Some(vec!["en".into()]),
                runtime: Some("ort".into()),
                quantization: Some("INT8".into()),
                ..Default::default()
            },
            ASRModel {
                id: "correction:flanec-base".into(),
                engine_id: "correction".into(),
                label: "FlanEC Base".into(),
                filename: "flanec-base".into(),
                url: String::new(),
                size: 275_887_316,
                storage_dir: jona_types::engine_storage_dir("correction"),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder_model_int8.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-base-onnx/resolve/main/encoder_model_int8.onnx"),
                            size: 110_057_102,
                        },
                        DownloadFile {
                            filename: "decoder_model_int8.onnx".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-base-onnx/resolve/main/decoder_model_int8.onnx"),
                            size: 163_406_470,
                        },
                        DownloadFile {
                            filename: "config.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-base-onnx/resolve/main/config.json"),
                            size: 1_510,
                        },
                        DownloadFile {
                            filename: "tokenizer.json".into(),
                            url: format!("{HF_BASE}/jonawhisper-flanec-base-onnx/resolve/main/tokenizer.json"),
                            size: 2_422_234,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                recommended_for: Some(vec!["en".into()]),
                params: Some(0.25),
                ram: Some(400_000_000),
                lang_codes: Some(vec!["en".into()]),
                runtime: Some("ort".into()),
                quantization: Some("INT8".into()),
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
