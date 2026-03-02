use crate::engines::{openai_api, EngineCatalog, EngineError};
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
        return openai_api::transcribe(provider, &asr_cloud_model, audio_path, &language);
    }

    // Local engine dispatch
    let catalog = EngineCatalog::new();

    let model = catalog.model_by_id(&model_id)
        .ok_or_else(|| EngineError::ModelNotFound(model_id.clone()))?;

    if !model.is_downloaded() {
        return Err(EngineError::ModelNotFound(model.local_path().display().to_string()));
    }

    // Dispatch to the appropriate native engine
    match model.engine_id.as_str() {
        "whisper" => crate::engines::whisper::transcribe_native(state, &model, audio_path, &language),
        "canary" => crate::canary_asr::transcribe(state, &model, audio_path, &language),
        "parakeet" => crate::parakeet_asr::transcribe(state, &model, audio_path, &language),
        "qwen-asr" => crate::qwen_asr::transcribe(state, &model, audio_path, &language),
        _ => Err(EngineError::LaunchFailed(format!("Unknown engine: {}", model.engine_id))),
    }
}
