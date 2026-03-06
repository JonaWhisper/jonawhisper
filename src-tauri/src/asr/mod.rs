use crate::engines::{EngineCatalog, EngineError};
use crate::state::AppState;
use std::path::Path;

pub fn transcribe(
    state: &AppState,
    audio_path: &Path,
) -> Result<String, EngineError> {
    let (model_id, language, gpu_mode, asr_cloud_model, providers) = {
        let s = state.settings.lock().unwrap();
        (
            s.selected_model_id.clone(),
            s.selected_language.clone(),
            s.gpu_mode,
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

    // Local engine dispatch — fully dynamic via ASREngine trait
    let catalog = EngineCatalog::global();

    let model = catalog.model_by_id(&model_id)
        .ok_or_else(|| EngineError::ModelNotFound(model_id.clone()))?;

    if !model.is_downloaded() {
        return Err(EngineError::ModelNotFound(model.local_path().display().to_string()));
    }

    let engine = catalog.engine_by_id(&model.engine_id)
        .ok_or_else(|| EngineError::LaunchFailed(format!("Unknown engine: {}", model.engine_id)))?;

    let context_key = engine.context_key(&model, gpu_mode);

    state.contexts.run_with(
        &model.engine_id,
        &context_key,
        || engine.create_context(&model, gpu_mode),
        |ctx| engine.transcribe(ctx, audio_path, &language),
    )
}
