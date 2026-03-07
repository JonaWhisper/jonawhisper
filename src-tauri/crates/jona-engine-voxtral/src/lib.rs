use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineError, EngineRegistration,
    GpuMode, Language,
};
use std::any::Any;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::path::Path;

// -- Audio utility (inline) --

fn read_wav_f32(path: &Path) -> Result<Vec<f32>, EngineError> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to open WAV: {}", e)))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let samples_f32: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_val = (1u32 << (bits - 1)) as f32;
            reader.into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => {
            reader.into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect()
        }
    };
    if channels > 1 {
        Ok(samples_f32.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect())
    } else {
        Ok(samples_f32)
    }
}

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

// -- Context (cached model state) --

/// Cached Voxtral inference context wrapping the C voxtral library.
pub struct VoxtralContext {
    ctx: *mut VoxCtx,
}

unsafe impl Send for VoxtralContext {} // protected by ContextMap Mutex

impl Drop for VoxtralContext {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { vox_free(self.ctx) };
        }
    }
}

// -- Loading --

/// Load a Voxtral model from a directory.
pub fn load(model_dir: &Path) -> Result<VoxtralContext, EngineError> {
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
    })
}

// -- Inference --

/// Transcribe an audio file using a loaded VoxtralContext.
pub fn transcribe(ctx: &mut VoxtralContext, audio_path: &Path, _language: &str) -> Result<String, EngineError> {
    let samples = read_wav_f32(audio_path)?;

    let result_ptr = unsafe {
        vox_transcribe_audio(ctx.ctx, samples.as_ptr(), samples.len() as c_int)
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

// -- Engine (catalogue) --

pub struct VoxtralEngine;

fn storage_dir() -> String {
    jona_types::models_dir().join("voxtral").to_string_lossy().to_string()
}

const HF_BASE: &str = "https://huggingface.co/mistralai/Voxtral-Mini-4B-Realtime-2602/resolve/main/";

impl ASREngine for VoxtralEngine {
    fn engine_id(&self) -> &str { "voxtral" }
    fn display_name(&self) -> &str { "Voxtral" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "voxtral:mini-4b-realtime".into(),
                engine_id: "voxtral".into(),
                label: "Voxtral Realtime 4B".into(),
                filename: "mini-4b-realtime".into(),
                url: String::new(),
                size: 8_859_462_744 + 14_910_348 + 1_343,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "consolidated.safetensors".into(),
                            url: format!("{}consolidated.safetensors", HF_BASE),
                            size: 8_859_462_744,
                        },
                        DownloadFile {
                            filename: "tekken.json".into(),
                            url: format!("{}tekken.json", HF_BASE),
                            size: 14_910_348,
                        },
                        DownloadFile {
                            filename: "params.json".into(),
                            url: format!("{}params.json", HF_BASE),
                            size: 1_343,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                wer: Some(8.7),
                rtf: Some(0.40),
                recommended_for: None,
                params: Some(4.4),
                ram: Some(10_000_000_000),
                lang_codes: Some(vec![
                    "en".into(), "fr".into(), "de".into(), "es".into(), "it".into(),
                    "pt".into(), "nl".into(), "ru".into(), "pl".into(), "tr".into(),
                    "ja".into(), "ko".into(), "zh".into(),
                ]),
                runtime: Some("metal".into()),
                quantization: Some("BF16".into()),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "en".into(), label: "English".into() },
            Language { code: "fr".into(), label: "Fran\u{00e7}ais".into() },
            Language { code: "de".into(), label: "Deutsch".into() },
            Language { code: "es".into(), label: "Espa\u{00f1}ol".into() },
            Language { code: "it".into(), label: "Italiano".into() },
            Language { code: "pt".into(), label: "Portugu\u{00ea}s".into() },
            Language { code: "nl".into(), label: "Nederlands".into() },
            Language { code: "ru".into(), label: "\u{0420}\u{0443}\u{0441}\u{0441}\u{043a}\u{0438}\u{0439}".into() },
            Language { code: "pl".into(), label: "Polski".into() },
            Language { code: "tr".into(), label: "T\u{00fc}rk\u{00e7}e".into() },
            Language { code: "ja".into(), label: "\u{65e5}\u{672c}\u{8a9e}".into() },
            Language { code: "ko".into(), label: "\u{d55c}\u{ad6d}\u{c5b4}".into() },
            Language { code: "zh".into(), label: "\u{4e2d}\u{6587}".into() },
        ]
    }

    fn description(&self) -> &str {
        "Mistral Voxtral Realtime 4B. 13 languages, Metal GPU acceleration."
    }

    fn create_context(&self, model: &ASRModel, _gpu_mode: GpuMode)
        -> Result<Box<dyn Any + Send>, EngineError>
    {
        let ctx = load(&model.local_path())?;
        Ok(Box::new(ctx))
    }

    fn transcribe(&self, ctx: &mut dyn Any, audio_path: &Path, language: &str)
        -> Result<String, EngineError>
    {
        let ctx = ctx.downcast_mut::<VoxtralContext>()
            .ok_or_else(|| EngineError::LaunchFailed("Invalid voxtral context".into()))?;
        transcribe(ctx, audio_path, language)
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(VoxtralEngine) }
}
