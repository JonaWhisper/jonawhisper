use jona_types::{
    ASREngine, ASRModel, DownloadType, EngineError, EngineRegistration, Language, GpuMode,
    common_languages,
};
use std::any::Any;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn inference_threads() -> usize {
    std::thread::available_parallelism().map(|p| p.get()).unwrap_or(4)
}

// -- Context (cached model state) --

/// Wrapper around `whisper_rs::WhisperContext` carrying gpu_mode for cache invalidation.
pub struct WhisperCtx {
    pub context: WhisperContext,
    pub gpu_mode: GpuMode,
}

// -- Inference --

/// Load a Whisper model into a context.
pub fn load(model_path: &Path, gpu_mode: GpuMode) -> Result<WhisperCtx, EngineError> {
    let use_gpu = gpu_mode != GpuMode::Cpu;
    log::info!("Loading whisper model: {} (gpu_mode={:?})", model_path.display(), gpu_mode);
    let mut ctx_params = WhisperContextParameters::default();
    ctx_params.use_gpu(use_gpu);
    ctx_params.flash_attn(true);
    let wctx = WhisperContext::new_with_params(
        &model_path.to_string_lossy(),
        ctx_params,
    ).map_err(|e| EngineError::LaunchFailed(format!("Failed to load whisper model: {}", e)))?;
    log::info!("Whisper model loaded: {} (gpu={})", model_path.display(), use_gpu);
    Ok(WhisperCtx {
        context: wctx,
        gpu_mode,
    })
}

/// Transcribe an audio file using a loaded WhisperCtx.
pub fn transcribe(ctx: &WhisperCtx, audio_path: &Path, language: &str) -> Result<String, EngineError> {
    let mut wstate = ctx.context.create_state()
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to create whisper state: {}", e)))?;

    let samples = jona_engines::audio::read_wav_f32(audio_path)?;

    let n_threads = inference_threads() as i32;
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

    wstate.full(params, &samples)
        .map_err(|e| EngineError::LaunchFailed(format!("Whisper transcription failed: {}", e)))?;

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

// -- Engine (catalogue) --

pub struct WhisperEngine;

impl ASREngine for WhisperEngine {
    fn engine_id(&self) -> &str { "whisper" }
    fn display_name(&self) -> &str { "Whisper" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "whisper:large-v3".into(), engine_id: "whisper".into(),
                label: "Whisper Large V3".into(), filename: "ggml-large-v3.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin".into(),
                size: 3_100_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(1.8), rtf: Some(0.50),
                params: Some(1.55), ram: Some(4_000_000_000),
                quantization: Some("FP16".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v2".into(), engine_id: "whisper".into(),
                label: "Whisper Large V2".into(), filename: "ggml-large-v2.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v2.bin".into(),
                size: 3_090_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(1.9), rtf: Some(0.50),
                params: Some(1.55), ram: Some(4_000_000_000),
                quantization: Some("FP16".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v3-turbo".into(), engine_id: "whisper".into(),
                label: "Whisper V3 Turbo".into(), filename: "ggml-large-v3-turbo.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".into(),
                size: 1_600_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.1), rtf: Some(0.25),
                params: Some(0.81), ram: Some(2_500_000_000),
                recommended_for: Some(vec![]),
                quantization: Some("FP16".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v3-turbo-q8".into(), engine_id: "whisper".into(),
                label: "Whisper V3 Turbo".into(), filename: "ggml-large-v3-turbo-q8_0.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q8_0.bin".into(),
                size: 874_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.1), rtf: Some(0.20),
                params: Some(0.81), ram: Some(1_300_000_000),
                quantization: Some("Q8".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v3-turbo-q5".into(), engine_id: "whisper".into(),
                label: "Whisper V3 Turbo".into(), filename: "ggml-large-v3-turbo-q5_0.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin".into(),
                size: 574_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.3), rtf: Some(0.15),
                params: Some(0.81), ram: Some(900_000_000),
                quantization: Some("Q5".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v3-french-distil".into(), engine_id: "whisper".into(),
                label: "Whisper V3 French".into(), filename: "ggml-model-q5_0.bin".into(),
                url: "https://huggingface.co/bofenghuang/whisper-large-v3-french-distil-dec2/resolve/main/ggml-model-q5_0.bin".into(),
                size: 538_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(1.5), rtf: Some(0.20),
                params: Some(0.76), ram: Some(900_000_000),
                lang_codes: Some(vec!["fr".into()]),
                quantization: Some("Q5".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:medium".into(), engine_id: "whisper".into(),
                label: "Whisper Medium".into(), filename: "ggml-medium.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".into(),
                size: 1_500_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.7), rtf: Some(0.35),
                params: Some(0.77), ram: Some(2_000_000_000),
                quantization: Some("FP16".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:medium-q5".into(), engine_id: "whisper".into(),
                label: "Whisper Medium".into(), filename: "ggml-medium-q5_0.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium-q5_0.bin".into(),
                size: 539_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.8), rtf: Some(0.20),
                params: Some(0.77), ram: Some(900_000_000),
                quantization: Some("Q5".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:small".into(), engine_id: "whisper".into(),
                label: "Whisper Small".into(), filename: "ggml-small.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
                size: 466_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(3.4), rtf: Some(0.15),
                params: Some(0.244), ram: Some(750_000_000),
                quantization: Some("FP16".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:small-q5".into(), engine_id: "whisper".into(),
                label: "Whisper Small".into(), filename: "ggml-small-q5_1.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin".into(),
                size: 190_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(3.6), rtf: Some(0.10),
                params: Some(0.244), ram: Some(400_000_000),
                quantization: Some("Q5".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:base".into(), engine_id: "whisper".into(),
                label: "Whisper Base".into(), filename: "ggml-base.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
                size: 142_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(5.0), rtf: Some(0.08),
                params: Some(0.074), ram: Some(300_000_000),
                quantization: Some("FP16".into()),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:tiny".into(), engine_id: "whisper".into(),
                label: "Whisper Tiny".into(), filename: "ggml-tiny.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".into(),
                size: 75_000_000, storage_dir: jona_types::engine_storage_dir("whisper"),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(7.6), rtf: Some(0.05),
                params: Some(0.039), ram: Some(200_000_000),
                quantization: Some("FP16".into()),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> { common_languages() }

    fn description(&self) -> &str {
        if cfg!(target_os = "macos") {
            "Native Whisper engine with Metal GPU acceleration."
        } else {
            "Native Whisper engine with CPU inference."
        }
    }

    fn context_key(&self, model: &ASRModel, gpu_mode: GpuMode) -> String {
        format!("{}:{:?}", model.id, gpu_mode)
    }

    fn create_context(&self, model: &ASRModel, gpu_mode: GpuMode)
        -> Result<Box<dyn Any + Send>, EngineError>
    {
        let ctx = load(&model.local_path(), gpu_mode)?;
        Ok(Box::new(ctx))
    }

    fn transcribe(&self, ctx: &mut dyn Any, audio_path: &Path, language: &str)
        -> Result<String, EngineError>
    {
        let ctx = ctx.downcast_ref::<WhisperCtx>()
            .ok_or_else(|| EngineError::LaunchFailed("Invalid whisper context".into()))?;
        transcribe(ctx, audio_path, language)
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(WhisperEngine) }
}
