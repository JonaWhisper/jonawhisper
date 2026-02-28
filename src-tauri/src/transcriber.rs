use crate::engines::{openai_api, EngineCatalog, EngineError};
use crate::state::AppState;
use std::path::Path;

pub fn transcribe(
    state: &AppState,
    audio_path: &Path,
) -> Result<String, EngineError> {
    let (model_id, language, asr_provider_id, asr_cloud_model, providers) = {
        let s = state.settings.lock().unwrap();
        (
            s.selected_model_id.clone(),
            s.selected_language.clone(),
            s.asr_provider_id.clone(),
            s.asr_cloud_model.clone(),
            s.providers.clone(),
        )
    };

    // Cloud dispatch: if a cloud ASR provider is selected, use it
    if !asr_provider_id.is_empty() {
        let provider = providers.iter().find(|p| p.id == asr_provider_id)
            .ok_or_else(|| EngineError::ApiError(
                format!("ASR provider '{}' not found", asr_provider_id)
            ))?;
        return openai_api::transcribe(provider, &asr_cloud_model, audio_path, &language);
    }

    // Local engine dispatch
    let catalog = EngineCatalog::new();

    let model = catalog.model_by_id(&model_id)
        .ok_or_else(|| EngineError::ModelNotFound(model_id.clone()))?;

    if !model.is_downloaded() {
        return Err(EngineError::ModelNotFound(model.local_path().display().to_string()));
    }

    // Native whisper-rs transcription
    crate::engines::whisper::transcribe_native(state, &model, audio_path, &language)
}
