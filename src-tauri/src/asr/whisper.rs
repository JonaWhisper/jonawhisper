use crate::engines::{ASRModel, EngineError};
use crate::state::{AppState, HasModelId};
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Wrapper around `whisper_rs::WhisperContext` carrying model_id + gpu_mode for cache invalidation.
pub struct WhisperCtx {
    pub context: WhisperContext,
    pub model_id: String,
    pub gpu_mode: String,
}

impl HasModelId for WhisperCtx {
    fn model_id(&self) -> &str {
        &self.model_id
    }
}

/// Native whisper-rs transcription — bypasses subprocess entirely.
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

    let model_path_str = model_path.to_string_lossy().to_string();
    let gpu_mode = state.settings.lock().unwrap().gpu_mode.clone();

    // Load or reuse cached WhisperContext (invalidate if model or gpu_mode changed)
    let mut ctx_guard = state.inference.asr.whisper.lock();
    if ctx_guard.as_ref().map_or(true, |w| w.model_id != model.id || w.gpu_mode != gpu_mode) {
        let use_gpu = gpu_mode != "cpu";
        log::info!("Loading whisper model: {} (gpu_mode={})", model.id, gpu_mode);
        let mut ctx_params = WhisperContextParameters::default();
        ctx_params.use_gpu(use_gpu);
        ctx_params.flash_attn(true);
        let wctx = WhisperContext::new_with_params(
            &model_path_str,
            ctx_params,
        ).map_err(|e| EngineError::LaunchFailed(format!("Failed to load whisper model: {}", e)))?;
        *ctx_guard = Some(WhisperCtx {
            context: wctx,
            model_id: model.id.clone(),
            gpu_mode: gpu_mode.clone(),
        });
        log::info!("Whisper model loaded: {} (gpu={})", model.id, use_gpu);
    }

    let ctx = &ctx_guard.as_ref().unwrap().context;

    // Create a lightweight state for this transcription
    let mut wstate = ctx.create_state()
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to create whisper state: {}", e)))?;

    // Read WAV audio as f32 mono 16kHz
    let audio = crate::audio::read_wav_f32(audio_path)?;

    // Configure transcription parameters
    let n_threads = std::thread::available_parallelism()
        .map(|p| p.get() as i32)
        .unwrap_or(4);
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(n_threads);
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_no_timestamps(true);

    if language != "auto" {
        params.set_language(Some(language));
    } else {
        params.set_detect_language(true);
    }

    // Run transcription
    wstate.full(params, &audio)
        .map_err(|e| EngineError::LaunchFailed(format!("Whisper transcription failed: {}", e)))?;

    // Extract text from segments
    let mut text = String::new();
    let n_segments = wstate.full_n_segments();
    for i in 0..n_segments {
        if let Some(segment) = wstate.get_segment(i) {
            if let Ok(s) = segment.to_str() {
                text.push_str(s);
            }
        }
    }

    Ok(text.trim().to_string())
}
