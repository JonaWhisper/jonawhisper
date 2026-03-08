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

#[cfg(test)]
mod tests {
    use super::*;
    use jona_types::ASREngine;

    #[test]
    fn engine_registers_as_punctuation() {
        // BERT punctuation is a cleanup engine, not ASR — it must register under
        // the Punctuation category so the UI places it in the cleanup section.
        let engine = BertPunctuationEngine;
        assert_eq!(engine.engine_id(), "bert-punctuation");
        assert_eq!(engine.category(), jona_types::EngineCategory::Punctuation);
    }

    #[test]
    fn punctuation_engine_does_not_pollute_language_selector() {
        // Punctuation engines should NOT list supported languages, otherwise
        // they would appear in the ASR language dropdown.
        let engine = BertPunctuationEngine;
        assert!(engine.supported_languages().is_empty());
    }

    #[test]
    fn user_can_pick_at_least_one_model() {
        let engine = BertPunctuationEngine;
        let models = engine.models();
        assert!(!models.is_empty(), "User must be able to choose at least one BERT model");
    }

    #[test]
    fn no_duplicate_models_in_picker() {
        let engine = BertPunctuationEngine;
        let models = engine.models();
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count, "Duplicate models would confuse the user");
    }

    #[test]
    fn all_download_urls_are_secure() {
        // Security: models must be downloaded over HTTPS to prevent MITM attacks.
        let engine = BertPunctuationEngine;
        for model in engine.models() {
            if !model.url.is_empty() {
                assert!(model.url.starts_with("https://"),
                    "Model {} has insecure download URL: {}", model.id, model.url);
            }
        }
    }

    #[test]
    fn models_report_size_for_download_progress() {
        // The download UI shows progress (bytes downloaded / total size).
        // A zero-size model would break percentage calculations.
        let engine = BertPunctuationEngine;
        for model in engine.models() {
            assert!(model.size > 0,
                "Model {} reports zero size, download progress UI would be broken", model.id);
        }
    }
}
