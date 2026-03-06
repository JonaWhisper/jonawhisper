pub use jona_engine_voxtral::VoxtralContext;

use crate::engines::{ASRModel, EngineError};
use crate::state::AppState;
use std::path::Path;

/// Voxtral transcription via jona-engine-voxtral crate.
pub fn transcribe(
    state: &AppState,
    model: &ASRModel,
    audio_path: &Path,
    language: &str,
) -> Result<String, EngineError> {
    let model_dir = model.local_path();
    if !model_dir.is_dir() {
        return Err(EngineError::ModelNotFound(model_dir.display().to_string()));
    }

    let model_id = model.id.clone();
    let mut ctx_guard = state.inference.asr.voxtral.get_or_load(&model_id, || {
        jona_engine_voxtral::load(&model_id, &model_dir)
    })?;
    let ctx = ctx_guard.as_mut().unwrap();

    jona_engine_voxtral::transcribe(ctx, audio_path, language)
}
