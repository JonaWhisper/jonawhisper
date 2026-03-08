pub mod ort_bert;
pub mod candle_bert;

use jona_types::{
    ASREngine, ASRModel, DownloadType, EngineCategory, EngineError, EngineRegistration,
    GpuMode, Language,
};
use std::any::Any;

pub use ort_bert::BertContext;
pub use candle_bert::CandlePunctContext;

pub struct BertPunctuationEngine;

impl ASREngine for BertPunctuationEngine {
    fn engine_id(&self) -> &str { "bert-punctuation" }
    fn display_name(&self) -> &str { "BERT Punctuation" }
    fn category(&self) -> EngineCategory { EngineCategory::Punctuation }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "bert-punctuation:fullstop-multilang-large".into(),
                engine_id: "bert-punctuation".into(),
                label: "Fullstop Multilang Large".into(),
                quantization: Some("INT8".into()),
                filename: "model_quantized.onnx".into(),
                url: "https://huggingface.co/ldenoue/fullstop-punctuation-multilang-large/resolve/main/onnx/model_quantized.onnx".into(),
                size: 562_000_000,
                storage_dir: jona_types::engine_storage_dir("bert"),
                download_type: DownloadType::SingleFile,
                download_marker: None,
                recommended_for: None,
                params: Some(0.56),
                ram: Some(600_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "de".into(), "it".into()]),
                runtime: Some("ort".into()),
                ..Default::default()
            },
            ASRModel {
                id: "bert-punctuation:fullstop-multilingual-base".into(),
                engine_id: "bert-punctuation".into(),
                label: "Fullstop Multilingual Base".into(),
                quantization: Some("FP32".into()),
                filename: "model.safetensors".into(),
                url: "https://huggingface.co/oliverguhr/fullstop-punctuation-multilingual-base/resolve/main/model.safetensors".into(),
                size: 1_112_000_000,
                storage_dir: jona_types::engine_storage_dir("bert"),
                download_type: DownloadType::SingleFile,
                download_marker: None,
                recommended_for: None,
                params: Some(0.28),
                ram: Some(560_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "de".into(), "it".into(), "nl".into()]),
                runtime: Some("candle".into()),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![]
    }

    fn description(&self) -> &str {
        "BERT-based punctuation restoration. Fast (~100ms), adds periods, commas, question marks."
    }

    fn create_context(&self, model: &ASRModel, _gpu_mode: GpuMode)
        -> Result<Box<dyn Any + Send>, EngineError>
    {
        let path = model.local_path();
        let runtime = model.runtime.as_deref().unwrap_or("ort");
        match runtime {
            "candle" => {
                let ctx = CandlePunctContext::load(&path)
                    .map_err(EngineError::LaunchFailed)?;
                Ok(Box::new(ctx))
            }
            _ => {
                let ctx = BertContext::load(&path)
                    .map_err(EngineError::LaunchFailed)?;
                Ok(Box::new(ctx))
            }
        }
    }

    fn cleanup(&self, ctx: &mut dyn Any, text: &str, _language: &str, _max_tokens: usize)
        -> Result<String, EngineError>
    {
        if let Some(ctx) = ctx.downcast_mut::<BertContext>() {
            return ort_bert::restore_punctuation(ctx, text)
                .map_err(|e| EngineError::LaunchFailed(e));
        }
        if let Some(ctx) = ctx.downcast_ref::<CandlePunctContext>() {
            return candle_bert::restore_punctuation(ctx, text)
                .map_err(|e| EngineError::LaunchFailed(e));
        }
        Err(EngineError::LaunchFailed("Invalid BERT context type".into()))
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(BertPunctuationEngine) }
}
