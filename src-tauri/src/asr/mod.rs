pub mod canary;
pub mod parakeet;
pub mod qwen;
pub mod voxtral;
pub mod whisper;

pub use canary::CanaryContext;
pub use parakeet::ParakeetContext;
pub use qwen::QwenContext;
pub use voxtral::VoxtralContext;
pub use whisper::WhisperCtx;

use crate::engines::{EngineCatalog, EngineError};
use crate::state::AppState;
use std::path::Path;

pub fn transcribe(
    state: &AppState,
    audio_path: &Path,
) -> Result<String, EngineError> {
    let (model_id, language, asr_cloud_model, providers) = {
        let s = state.settings.lock().unwrap();
        (
            s.selected_model_id.clone(),
            s.selected_language.clone(),
            s.asr_cloud_model.clone(),
            s.providers.clone(),
        )
    };

    // Cloud dispatch: selected_model_id = "cloud:<provider_id>"
    if let Some(provider_id) = model_id.strip_prefix("cloud:") {
        let provider = providers.iter().find(|p| p.id == provider_id)
            .ok_or_else(|| EngineError::ApiError(
                format!("ASR provider '{}' not found", provider_id)
            ))?;
        if !provider.has_asr() {
            return Err(EngineError::ApiError(
                format!("Provider '{}' does not support ASR transcription", provider.name)
            ));
        }
        return jona_provider::backend(provider.kind)
            .transcribe(provider, &asr_cloud_model, audio_path, &language)
            .map_err(|e| EngineError::ApiError(e.to_string()));
    }

    // Local engine dispatch
    let catalog = EngineCatalog::global();

    let model = catalog.model_by_id(&model_id)
        .ok_or_else(|| EngineError::ModelNotFound(model_id.clone()))?;

    if !model.is_downloaded() {
        return Err(EngineError::ModelNotFound(model.local_path().display().to_string()));
    }

    // Dispatch to the appropriate native engine
    match model.engine_id.as_str() {
        "whisper" => whisper::transcribe_native(state, &model, audio_path, &language),
        "canary" => canary::transcribe(state, &model, audio_path, &language),
        "parakeet" => parakeet::transcribe(state, &model, audio_path, &language),
        "qwen-asr" => qwen::transcribe(state, &model, audio_path, &language),
        "voxtral" => voxtral::transcribe(state, &model, audio_path, &language),
        _ => Err(EngineError::LaunchFailed(format!("Unknown engine: {}", model.engine_id))),
    }
}
