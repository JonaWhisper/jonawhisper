use super::*;
use crate::state::AppState;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperEngine;

impl ASREngine for WhisperEngine {
    fn engine_id(&self) -> &str { "whisper" }
    fn display_name(&self) -> &str { "Whisper" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "whisper:tiny".into(), engine_id: "whisper".into(),
                label: "Tiny".into(), filename: "ggml-tiny.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".into(),
                size: 75_000_000, storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(7.6), rtf: Some(0.05),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:base".into(), engine_id: "whisper".into(),
                label: "Base".into(), filename: "ggml-base.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
                size: 142_000_000, storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(5.0), rtf: Some(0.08),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:small".into(), engine_id: "whisper".into(),
                label: "Small".into(), filename: "ggml-small.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
                size: 466_000_000, storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(3.4), rtf: Some(0.15),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:medium".into(), engine_id: "whisper".into(),
                label: "Medium".into(), filename: "ggml-medium.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".into(),
                size: 1_500_000_000, storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.7), rtf: Some(0.35),
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v3-turbo".into(), engine_id: "whisper".into(),
                label: "Large V3 Turbo".into(), filename: "ggml-large-v3-turbo.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".into(),
                size: 1_600_000_000, storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(2.1), rtf: Some(0.25),
                recommended: true,
                ..Default::default()
            },
            ASRModel {
                id: "whisper:large-v3".into(), engine_id: "whisper".into(),
                label: "Large V3".into(), filename: "ggml-large-v3.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin".into(),
                size: 3_100_000_000, storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
                wer: Some(1.8), rtf: Some(0.50),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> { common_languages() }

    fn description(&self) -> &str { "Native Whisper engine with Metal GPU acceleration." }
    fn install_hint(&self) -> &str { "Built-in, no installation needed." }

    fn resolve_executable(&self) -> Option<String> {
        Some("built-in".into())
    }

    fn transcribe(&self, _model: &ASRModel, _audio_path: &Path, _language: &str) -> Result<String, EngineError> {
        Err(EngineError::LaunchFailed("Use transcribe_native() for whisper engine".into()))
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

    // Load or reuse cached WhisperContext
    let mut ctx_guard = state.whisper_context.lock().unwrap();
    if ctx_guard.as_ref().map_or(true, |(id, _)| id != &model.id) {
        log::info!("Loading whisper model: {}", model.id);
        let ctx = WhisperContext::new_with_params(
            &model_path_str,
            WhisperContextParameters::default(),
        ).map_err(|e| EngineError::LaunchFailed(format!("Failed to load whisper model: {}", e)))?;
        *ctx_guard = Some((model.id.clone(), ctx));
        log::info!("Whisper model loaded: {}", model.id);
    }

    let (_, ctx) = ctx_guard.as_ref().unwrap();

    // Create a lightweight state for this transcription
    let mut wstate = ctx.create_state()
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to create whisper state: {}", e)))?;

    // Read WAV audio as f32 mono 16kHz
    let audio = read_wav_f32(audio_path)?;

    // Configure transcription parameters
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
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
