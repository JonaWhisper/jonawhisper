use crate::engines::{EngineCatalog, EngineError};
use crate::state::AppState;
use std::path::Path;

pub fn transcribe(
    state: &AppState,
    audio_path: &Path,
) -> Result<String, EngineError> {
    let model_id = state.selected_model_id.lock().unwrap().clone();
    let language = state.selected_language.lock().unwrap().clone();
    let api_servers = state.api_servers.lock().unwrap().clone();

    let catalog = EngineCatalog::new(&api_servers);

    let model = catalog.model_by_id(&model_id)
        .ok_or_else(|| EngineError::ModelNotFound(model_id.clone()))?;

    if !model.is_downloaded() {
        return Err(EngineError::ModelNotFound(model.local_path().display().to_string()));
    }

    let engine = catalog.engine_for_model(&model)
        .ok_or_else(|| EngineError::EngineNotFound(model.engine_id.clone()))?;

    if engine.resolve_executable().is_none() && !model.is_remote_api() {
        return Err(EngineError::EngineUnavailable {
            engine_id: engine.engine_id().to_string(),
            install_hint: engine.install_hint().to_string(),
        });
    }

    engine.transcribe(&model, audio_path, &language)
}
