//! Voxtral Realtime 4B inference via vendored voxtral.c (pure C + Metal GPU).
//!
//! Pipeline: WAV → float32 samples → voxtral encoder/decoder → text.
//! Uses the batch convenience API `vox_transcribe_audio()`.

use crate::engines::{ASRModel, EngineError};
use crate::state::AppState;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::path::Path;

// -- FFI declarations (voxtral.h / voxtral_metal.h) --

#[repr(C)]
pub struct VoxCtx {
    _opaque: [u8; 0],
}

extern "C" {
    fn vox_load(model_dir: *const c_char) -> *mut VoxCtx;
    fn vox_free(ctx: *mut VoxCtx);
    fn vox_transcribe_audio(ctx: *mut VoxCtx, samples: *const f32, n_samples: c_int) -> *mut c_char;
    fn vox_metal_init() -> c_int;
    fn free(ptr: *mut c_void);
}

// -- Cached inference context --

pub struct VoxtralContext {
    ctx: *mut VoxCtx,
    pub model_id: String,
}

unsafe impl Send for VoxtralContext {} // protected by ContextSlot Mutex

impl Drop for VoxtralContext {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { vox_free(self.ctx) };
        }
    }
}

impl crate::state::HasModelId for VoxtralContext {
    fn model_id(&self) -> &str {
        &self.model_id
    }
}

/// Transcribe an audio file using Voxtral Realtime 4B.
pub fn transcribe(
    state: &AppState,
    model: &ASRModel,
    audio_path: &Path,
    _language: &str, // Voxtral auto-detects language, no API to force it
) -> Result<String, EngineError> {
    let model_dir = model.local_path();
    if !model_dir.is_dir() {
        return Err(EngineError::ModelNotFound(model_dir.display().to_string()));
    }

    let model_id = model.id.clone();
    let mut ctx_guard = state.inference.asr.voxtral.get_or_load(&model_id, || {
        log::info!("Loading Voxtral model from: {}", model_dir.display());

        // Initialize Metal GPU acceleration
        let metal_ok = unsafe { vox_metal_init() };
        if metal_ok == 1 {
            log::info!("Voxtral: Metal GPU initialized");
        } else {
            log::warn!("Voxtral: Metal unavailable, falling back to CPU");
        }

        let dir_cstr = std::ffi::CString::new(model_dir.to_string_lossy().as_bytes())
            .map_err(|e| EngineError::LaunchFailed(format!("Invalid path: {}", e)))?;

        let ctx = unsafe { vox_load(dir_cstr.as_ptr()) };
        if ctx.is_null() {
            return Err(EngineError::LaunchFailed(format!(
                "vox_load failed for {}",
                model_dir.display()
            )));
        }

        log::info!("Voxtral model loaded successfully");
        Ok(VoxtralContext {
            ctx,
            model_id: model_id.clone(),
        })
    })?;
    let voxtral = ctx_guard.as_mut().unwrap();

    // Read WAV audio (mono float32, resampled to 16kHz)
    let samples = crate::audio::read_wav_f32(audio_path)?;

    // Transcribe
    let result_ptr = unsafe {
        vox_transcribe_audio(voxtral.ctx, samples.as_ptr(), samples.len() as c_int)
    };
    if result_ptr.is_null() {
        return Err(EngineError::LaunchFailed("vox_transcribe_audio returned null".into()));
    }

    let text = unsafe { CStr::from_ptr(result_ptr) }
        .to_string_lossy()
        .to_string();

    // Free the malloc'd C string
    unsafe { free(result_ptr as *mut c_void) };

    Ok(text.trim().to_string())
}
