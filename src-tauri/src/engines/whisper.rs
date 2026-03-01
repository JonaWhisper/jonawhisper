use super::*;
use crate::state::AppState;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperVadParams};

pub struct WhisperEngine;

fn storage_dir() -> String {
    crate::state::models_dir().join("whisper").to_string_lossy().to_string()
}

impl ASREngine for WhisperEngine {
    fn engine_id(&self) -> &str { "whisper" }
    fn display_name(&self) -> &str { "Whisper" }

    fn models(&self) -> Vec<ASRModel> {
        // Sorted by WER ascending (best quality first)
        vec![
            ASRModel {
                id: "whisper:large-v3".into(), engine_id: "whisper".into(),
                label: "Large V3".into(), filename: "ggml-large-v3.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin".into(),
                size: 3_100_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(1.8), rtf: Some(0.50),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v2".into(), engine_id: "whisper".into(),
                label: "Large V2".into(), filename: "ggml-large-v2.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v2.bin".into(),
                size: 3_090_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(1.9), rtf: Some(0.50),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v3-turbo".into(), engine_id: "whisper".into(),
                label: "Large V3 Turbo".into(), filename: "ggml-large-v3-turbo.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".into(),
                size: 1_600_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.1), rtf: Some(0.25),
                recommended: true,
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v3-turbo-q8".into(), engine_id: "whisper".into(),
                label: "Large V3 Turbo Q8".into(), filename: "ggml-large-v3-turbo-q8_0.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q8_0.bin".into(),
                size: 874_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.1), rtf: Some(0.20),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v3-turbo-q5".into(), engine_id: "whisper".into(),
                label: "Large V3 Turbo Q5".into(), filename: "ggml-large-v3-turbo-q5_0.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin".into(),
                size: 574_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.3), rtf: Some(0.15),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:medium".into(), engine_id: "whisper".into(),
                label: "Medium".into(), filename: "ggml-medium.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".into(),
                size: 1_500_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.7), rtf: Some(0.35),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:medium-q5".into(), engine_id: "whisper".into(),
                label: "Medium Q5".into(), filename: "ggml-medium-q5_0.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium-q5_0.bin".into(),
                size: 539_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.8), rtf: Some(0.20),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:small".into(), engine_id: "whisper".into(),
                label: "Small".into(), filename: "ggml-small.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
                size: 466_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(3.4), rtf: Some(0.15),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:small-q5".into(), engine_id: "whisper".into(),
                label: "Small Q5".into(), filename: "ggml-small-q5_1.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin".into(),
                size: 190_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(3.6), rtf: Some(0.10),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:base".into(), engine_id: "whisper".into(),
                label: "Base".into(), filename: "ggml-base.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
                size: 142_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(5.0), rtf: Some(0.08),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:tiny".into(), engine_id: "whisper".into(),
                label: "Tiny".into(), filename: "ggml-tiny.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".into(),
                size: 75_000_000, storage_dir: storage_dir(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(7.6), rtf: Some(0.05),
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
    let mut ctx_guard = state.whisper_context.lock().unwrap();
    if ctx_guard.as_ref().map_or(true, |(id, mode, _)| id != &model.id || mode != &gpu_mode) {
        let use_gpu = gpu_mode != "cpu";
        log::info!("Loading whisper model: {} (gpu_mode={})", model.id, gpu_mode);
        let mut ctx_params = WhisperContextParameters::default();
        ctx_params.use_gpu(use_gpu);
        ctx_params.flash_attn(true);
        let ctx = WhisperContext::new_with_params(
            &model_path_str,
            ctx_params,
        ).map_err(|e| EngineError::LaunchFailed(format!("Failed to load whisper model: {}", e)))?;
        *ctx_guard = Some((model.id.clone(), gpu_mode.clone(), ctx));
        log::info!("Whisper model loaded: {} (gpu={})", model.id, use_gpu);
    }

    let (_, _, ctx) = ctx_guard.as_ref().unwrap();

    // Create a lightweight state for this transcription
    let mut wstate = ctx.create_state()
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to create whisper state: {}", e)))?;

    // Read WAV audio as f32 mono 16kHz
    let audio = read_wav_f32(audio_path)?;
    let duration_secs = audio.len() as f32 / 16000.0;
    log::info!("Audio: {} samples, {:.2}s, path={}", audio.len(), duration_secs, audio_path.display());

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

    // Enable VAD (Voice Activity Detection) to filter out noise
    if let Some(vad_path) = ensure_vad_model() {
        params.set_vad_model_path(Some(&vad_path));
        let mut vad_params = WhisperVadParams::new();
        vad_params.set_threshold(0.5);
        vad_params.set_min_speech_duration(250);
        vad_params.set_min_silence_duration(100);
        params.set_vad_params(vad_params);
        params.enable_vad(true);
        log::info!("VAD enabled");
    }

    // Suppress non-speech tokens (breathing, background noise tokens)
    params.set_suppress_nst(true);

    // Run transcription
    wstate.full(params, &audio)
        .map_err(|e| EngineError::LaunchFailed(format!("Whisper transcription failed: {}", e)))?;

    // Extract text from segments
    let mut text = String::new();
    let n_segments = wstate.full_n_segments();
    log::info!("Whisper: {} segments, {:.2}s audio", n_segments, duration_secs);
    for i in 0..n_segments {
        if let Some(segment) = wstate.get_segment(i) {
            if let Ok(s) = segment.to_str() {
                text.push_str(s);
            }
        }
    }
    Ok(text.trim().to_string())
}

/// Read a WAV file and convert to f32 mono samples.
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

    // Convert to mono by averaging channels
    if channels > 1 {
        Ok(samples_f32
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect())
    } else {
        Ok(samples_f32)
    }
}

const VAD_MODEL_URL: &str = "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v5.1.2.bin";
const VAD_MODEL_FILE: &str = "ggml-silero-vad.bin";

/// Return the path to the VAD model, downloading it if needed.
fn ensure_vad_model() -> Option<String> {
    let dir = crate::state::models_dir().join("vad");
    let path = dir.join(VAD_MODEL_FILE);
    if path.exists() {
        return Some(path.to_string_lossy().to_string());
    }

    log::info!("Downloading VAD model from {}", VAD_MODEL_URL);
    let _ = std::fs::create_dir_all(&dir);

    let bytes = reqwest::blocking::get(VAD_MODEL_URL).ok()?.bytes().ok()?;
    std::fs::write(&path, &bytes).ok()?;

    log::info!("VAD model downloaded to {}", path.display());
    Some(path.to_string_lossy().to_string())
}
