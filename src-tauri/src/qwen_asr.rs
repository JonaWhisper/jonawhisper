//! Qwen3-ASR inference wrapper around the `qwen-asr` crate.
//!
//! Pipeline: WAV → mel spectrogram → Whisper-style encoder-decoder → text.
//! Uses Apple Accelerate (vDSP + AMX) for hardware-accelerated matrix operations.

use crate::engines::{ASRModel, EngineError};
use crate::state::AppState;
use std::path::Path;

/// Language code (ISO 639-1) to Qwen3-ASR language name mapping.
fn lang_code_to_name(code: &str) -> Option<&'static str> {
    match code {
        "zh" => Some("Chinese"),
        "en" => Some("English"),
        "yue" => Some("Cantonese"),
        "ar" => Some("Arabic"),
        "de" => Some("German"),
        "fr" => Some("French"),
        "es" => Some("Spanish"),
        "pt" => Some("Portuguese"),
        "id" => Some("Indonesian"),
        "it" => Some("Italian"),
        "ko" => Some("Korean"),
        "ru" => Some("Russian"),
        "th" => Some("Thai"),
        "vi" => Some("Vietnamese"),
        "ja" => Some("Japanese"),
        "tr" => Some("Turkish"),
        "hi" => Some("Hindi"),
        "ms" => Some("Malay"),
        "nl" => Some("Dutch"),
        "sv" => Some("Swedish"),
        "da" => Some("Danish"),
        "fi" => Some("Finnish"),
        "pl" => Some("Polish"),
        "cs" => Some("Czech"),
        "fil" => Some("Filipino"),
        "fa" => Some("Persian"),
        "el" => Some("Greek"),
        "ro" => Some("Romanian"),
        "hu" => Some("Hungarian"),
        "mk" => Some("Macedonian"),
        _ => None,
    }
}

/// Cached Qwen3-ASR inference context.
pub struct QwenContext {
    ctx: qwen_asr::context::QwenCtx,
    pub model_id: String,
}

/// Transcribe an audio file using Qwen3-ASR.
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

    // Load or reuse cached context
    let mut ctx_guard = state.qwen_context.lock().unwrap();
    if ctx_guard.as_ref().map_or(true, |c| c.model_id != model.id) {
        log::info!("Loading Qwen3-ASR model: {}", model.id);
        let dir_str = model_dir.to_string_lossy().to_string();
        let qwen_ctx = qwen_asr::context::QwenCtx::load(&dir_str)
            .ok_or_else(|| EngineError::LaunchFailed(
                format!("Failed to load Qwen3-ASR from {}", model_dir.display())
            ))?;

        log::info!("Qwen3-ASR loaded, optimizations: {:?}", qwen_asr::optimization_flags());

        *ctx_guard = Some(QwenContext {
            ctx: qwen_ctx,
            model_id: model.id.clone(),
        });
    }
    let qwen = ctx_guard.as_mut().unwrap();

    // Set forced language if not auto
    if language != "auto" {
        if let Some(lang_name) = lang_code_to_name(language) {
            let _ = qwen.ctx.set_force_language(lang_name);
        }
    } else {
        let _ = qwen.ctx.set_force_language("");
    }

    // Read WAV audio
    let audio = crate::engines::whisper::read_wav_f32(audio_path)?;

    // Transcribe
    let text = qwen_asr::transcribe::transcribe_audio(&mut qwen.ctx, &audio)
        .ok_or_else(|| EngineError::LaunchFailed("Qwen3-ASR transcription failed".into()))?;

    log::debug!(
        "Qwen3-ASR: {:.0}ms total, {:.0}ms encode, {:.0}ms decode, {} tokens",
        qwen.ctx.perf_total_ms,
        qwen.ctx.perf_encode_ms,
        qwen.ctx.perf_decode_ms,
        qwen.ctx.perf_text_tokens,
    );

    Ok(text.trim().to_string())
}
