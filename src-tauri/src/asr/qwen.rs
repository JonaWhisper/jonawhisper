pub use jona_engine_qwen::QwenContext;

use crate::engines::{ASRModel, EngineError};
use crate::state::AppState;
use std::path::Path;

/// Qwen3-ASR transcription via jona-engine-qwen crate.
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
    let mut ctx_guard = state.inference.asr.qwen.get_or_load(&model_id, || {
        jona_engine_qwen::load(&model_id, &model_dir)
    })?;
    let qwen = ctx_guard.as_mut().unwrap();

    jona_engine_qwen::transcribe(qwen, audio_path, language)
}
