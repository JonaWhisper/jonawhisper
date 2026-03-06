pub use jona_engine_whisper::WhisperCtx;

use crate::engines::{ASRModel, EngineError};
use crate::state::AppState;
use std::path::Path;

/// Native whisper-rs transcription via jona-engine-whisper crate.
pub fn transcribe_native(
    state: &AppState,
    model: &ASRModel,
    audio_path: &Path,
    language: &str,
) -> Result<String, EngineError> {
    let model_path = model.local_path();
    if !model_path.exists() {
        return Err(EngineError::ModelNotFound(model_path.display().to_string()));
    }

    let gpu_mode = state.settings.lock().unwrap().gpu_mode;

    // Load or reuse cached WhisperContext (invalidate if model or gpu_mode changed)
    let mut ctx_guard = state.inference.asr.whisper.lock();
    if ctx_guard.as_ref().map_or(true, |w| w.model_id != model.id || w.gpu_mode != gpu_mode) {
        *ctx_guard = Some(jona_engine_whisper::load(&model.id, &model_path, gpu_mode)?);
    }

    let ctx = ctx_guard.as_ref().unwrap();
    jona_engine_whisper::transcribe(ctx, audio_path, language)
}
