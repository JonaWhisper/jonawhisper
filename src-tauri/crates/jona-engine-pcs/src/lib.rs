mod inference;

use jona_types::{
    ASREngine, ASRModel, DownloadType, EngineCategory, EngineError, EngineRegistration,
    GpuMode, Language,
};
use std::any::Any;

pub use inference::{PcsContext, restore_punctuation_and_case};

pub struct PcsPunctuationEngine;

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
                storage_dir: jona_types::engine_storage_dir("pcs"),
                download_type: DownloadType::SingleFile,
                download_marker: None,
                recommended_for: Some(vec![]),
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

    fn create_context(&self, model: &ASRModel, _gpu_mode: GpuMode)
        -> Result<Box<dyn Any + Send>, EngineError>
    {
        let ctx = PcsContext::load(&model.local_path())
            .map_err(EngineError::LaunchFailed)?;
        Ok(Box::new(ctx))
    }

    fn cleanup(&self, ctx: &mut dyn Any, text: &str, _language: &str, _max_tokens: usize)
        -> Result<String, EngineError>
    {
        let ctx = ctx.downcast_mut::<PcsContext>()
            .ok_or_else(|| EngineError::LaunchFailed("Invalid PCS context".into()))?;
        restore_punctuation_and_case(ctx, text)
            .map_err(EngineError::LaunchFailed)
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(PcsPunctuationEngine) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jona_types::ASREngine;

    #[test]
    fn engine_registers_as_punctuation() {
        // PCS is a punctuation cleanup engine, not ASR.
        let engine = PcsPunctuationEngine;
        assert_eq!(engine.engine_id(), "pcs-punctuation");
        assert_eq!(engine.category(), jona_types::EngineCategory::Punctuation);
    }

    #[test]
    fn punctuation_engine_does_not_pollute_language_selector() {
        let engine = PcsPunctuationEngine;
        assert!(engine.supported_languages().is_empty());
    }

    #[test]
    fn user_can_pick_at_least_one_model() {
        let engine = PcsPunctuationEngine;
        assert!(!engine.models().is_empty(), "User must be able to choose at least one PCS model");
    }

    #[test]
    fn no_duplicate_models_in_picker() {
        let engine = PcsPunctuationEngine;
        let models = engine.models();
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count, "Duplicate models would confuse the user");
    }

    #[test]
    fn all_download_urls_are_secure() {
        let engine = PcsPunctuationEngine;
        for model in engine.models() {
            if !model.url.is_empty() {
                assert!(model.url.starts_with("https://"),
                    "Model {} has insecure download URL: {}", model.id, model.url);
            }
        }
    }

    #[test]
    fn models_report_size_for_download_progress() {
        let engine = PcsPunctuationEngine;
        for model in engine.models() {
            assert!(model.size > 0,
                "Model {} reports zero size, download progress UI would be broken", model.id);
        }
    }
}
