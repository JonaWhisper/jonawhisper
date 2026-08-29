use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineError, EngineRegistration,
    GpuMode, Language, TranscriptionResult,
};
use std::any::Any;
use std::marker::PhantomData;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::path::Path;

// -- FFI declarations (voxtral.h / voxtral_metal.h) --

#[repr(C)]
pub struct VoxCtx {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct VoxStream {
    _opaque: [u8; 0],
}

extern "C" {
    fn vox_load(model_dir: *const c_char) -> *mut VoxCtx;
    fn vox_free(ctx: *mut VoxCtx);
    fn vox_transcribe_audio(ctx: *mut VoxCtx, samples: *const f32, n_samples: c_int) -> *mut c_char;
    fn free(ptr: *mut c_void);

    fn vox_stream_init(ctx: *mut VoxCtx) -> *mut VoxStream;
    fn vox_stream_feed(s: *mut VoxStream, samples: *const f32, n_samples: c_int) -> c_int;
    fn vox_stream_finish(s: *mut VoxStream) -> c_int;
    fn vox_stream_get(s: *mut VoxStream, out_tokens: *mut *const c_char, max: c_int) -> c_int;
    fn vox_stream_free(s: *mut VoxStream);
    fn vox_set_processing_interval(s: *mut VoxStream, seconds: f32);
}

// Defined in voxtral_metal.m, which only builds on macOS.
#[cfg(target_os = "macos")]
extern "C" {
    fn vox_metal_init() -> c_int;
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
    #[cfg(target_os = "macos")]
    {
        if unsafe { vox_metal_init() } == 1 {
            log::info!("Voxtral: Metal GPU initialized");
        } else {
            log::warn!("Voxtral: Metal unavailable, falling back to CPU");
        }
    }
    #[cfg(not(target_os = "macos"))]
    log::info!("Voxtral: CPU inference (Metal is macOS-only)");

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

// -- Streaming --

/// How many token pointers `vox_stream_get` fills per call.
const DRAIN_BATCH: usize = 32;

/// Incremental transcription over one recording: feed audio as it arrives, read
/// text back before the user stops speaking.
///
/// Borrows the context because the C stream holds a pointer into it — the
/// borrow is what stops the model being freed while a stream is still live.
pub struct VoxtralStream<'a> {
    stream: *mut VoxStream,
    ctx: PhantomData<&'a mut VoxtralContext>,
}

// Same reasoning as VoxtralContext: reachable only through the ContextMap mutex.
unsafe impl Send for VoxtralStream<'_> {}

impl<'a> VoxtralStream<'a> {
    pub fn new(ctx: &'a mut VoxtralContext) -> Result<Self, EngineError> {
        let stream = unsafe { vox_stream_init(ctx.ctx) };
        if stream.is_null() {
            return Err(EngineError::LaunchFailed("vox_stream_init returned null".into()));
        }
        Ok(Self { stream, ctx: PhantomData })
    }

    /// Minimum seconds of audio between encoder runs. Lower is more responsive
    /// and costs more GPU; the C default is 2.0, and the first chunk always
    /// waits for ~3s because the decoder prompt needs 312 mel frames.
    pub fn set_interval(&mut self, seconds: f32) {
        unsafe { vox_set_processing_interval(self.stream, seconds) };
    }

    /// Push mono f32 samples at 16 kHz. Runs the encoder and decoder on whatever
    /// is available, queueing any text produced.
    pub fn feed(&mut self, samples: &[f32]) -> Result<(), EngineError> {
        if samples.is_empty() {
            return Ok(());
        }
        let rc = unsafe {
            vox_stream_feed(self.stream, samples.as_ptr(), samples.len() as c_int)
        };
        if rc != 0 {
            return Err(EngineError::LaunchFailed("vox_stream_feed failed".into()));
        }
        Ok(())
    }

    /// Text decoded since the last call, empty when nothing new is ready.
    pub fn drain(&mut self) -> String {
        let mut out = String::new();
        loop {
            let mut slots: [*const c_char; DRAIN_BATCH] = [std::ptr::null(); DRAIN_BATCH];
            let n = unsafe {
                vox_stream_get(self.stream, slots.as_mut_ptr(), DRAIN_BATCH as c_int)
            };
            if n <= 0 {
                return out;
            }
            for slot in slots.iter().take(n as usize) {
                if slot.is_null() {
                    continue;
                }
                // Owned by the stream and valid until vox_stream_free: copy, never free.
                out.push_str(&unsafe { CStr::from_ptr(*slot) }.to_string_lossy());
            }
        }
    }

    /// Flush the trailing audio and return everything still queued.
    pub fn finish(&mut self) -> Result<String, EngineError> {
        let rc = unsafe { vox_stream_finish(self.stream) };
        if rc != 0 {
            return Err(EngineError::LaunchFailed("vox_stream_finish failed".into()));
        }
        Ok(self.drain())
    }
}

impl Drop for VoxtralStream<'_> {
    fn drop(&mut self) {
        unsafe { vox_stream_free(self.stream) };
    }
}

// -- Inference --

/// Transcribe an audio file using a loaded VoxtralContext.
pub fn transcribe(ctx: &mut VoxtralContext, audio_path: &Path, _language: &str) -> Result<String, EngineError> {
    let samples = jona_engines::audio::read_wav_f32(audio_path)?;

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
                storage_dir: jona_types::engine_storage_dir("voxtral"),
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
        -> Result<TranscriptionResult, EngineError>
    {
        let ctx = ctx.downcast_mut::<VoxtralContext>()
            .ok_or_else(|| EngineError::LaunchFailed("Invalid voxtral context".into()))?;
        let text = transcribe(ctx, audio_path, language)?;
        Ok(TranscriptionResult::text_only(text))
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(VoxtralEngine) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jona_types::{ASREngine, DownloadType};

    /// Exercises the streaming FFI end to end. Ignored by default: it needs the
    /// 8.9 GB model on disk, so CI can never run it. Run with
    /// `cargo test -p jona-engine-voxtral -- --ignored --nocapture` once the
    /// model is installed.
    #[test]
    #[ignore]
    fn stream_transcribes_a_silent_buffer() {
        let model = VoxtralEngine.models().remove(0);
        let dir = model.local_path();
        if !dir.join("consolidated.safetensors").exists() {
            eprintln!("SKIPPED: Voxtral model not installed at {}", dir.display());
            return;
        }

        let mut ctx = load(&dir).expect("model should load");
        let mut stream = VoxtralStream::new(&mut ctx).expect("stream should start");
        stream.set_interval(0.5);

        // 2 s of silence at 16 kHz: enough to drive a full feed/finish cycle.
        let silence = vec![0.0f32; 32_000];
        stream.feed(&silence).expect("feed should succeed");
        let _partial = stream.drain();
        let text = stream.finish().expect("finish should succeed");
        println!("STREAM OUT|{text}");
    }

    #[test]
    fn engine_registers_as_asr() {
        let engine = VoxtralEngine;
        assert_eq!(engine.engine_id(), "voxtral");
        assert_eq!(engine.category(), jona_types::EngineCategory::ASR);
    }

    #[test]
    fn user_can_pick_at_least_one_model() {
        let engine = VoxtralEngine;
        assert!(!engine.models().is_empty(), "User must be able to choose at least one Voxtral model");
    }

    #[test]
    fn no_duplicate_models_in_picker() {
        let engine = VoxtralEngine;
        let models = engine.models();
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count, "Duplicate models would confuse the user");
    }

    #[test]
    fn all_download_urls_are_secure() {
        let engine = VoxtralEngine;
        for model in engine.models() {
            if let DownloadType::MultiFile { files } = &model.download_type {
                for file in files {
                    assert!(file.url.starts_with("https://"),
                        "Model {} file {} has insecure download URL: {}", model.id, file.filename, file.url);
                }
            }
        }
    }

    #[test]
    fn models_report_size_for_download_progress() {
        let engine = VoxtralEngine;
        for model in engine.models() {
            assert!(model.size > 0,
                "Model {} reports zero size, download progress UI would be broken", model.id);
        }
    }

    #[test]
    fn voxtral_supports_multiple_languages() {
        // Voxtral supports 13 languages.
        let engine = VoxtralEngine;
        let langs = engine.supported_languages();
        assert!(langs.len() >= 10, "Voxtral should support at least 10 languages, got {}", langs.len());
    }
}
