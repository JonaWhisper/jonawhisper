use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineError, HasModelId, Language,
};
use std::path::Path;

// -- Audio utility (inline, avoids dependency on jona-engines) --

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

// -- Language mapping --

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

// -- Context (cached model state) --

/// Cached Qwen3-ASR inference context.
pub struct QwenContext {
    ctx: qwen_asr::context::QwenCtx,
    pub model_id: String,
}

impl HasModelId for QwenContext {
    fn model_id(&self) -> &str {
        &self.model_id
    }
}

// -- Inference --

/// Load a Qwen3-ASR model into a context.
pub fn load(model_id: &str, model_dir: &Path) -> Result<QwenContext, EngineError> {
    log::info!("Loading Qwen3-ASR model: {}", model_id);
    let dir_str = model_dir.to_string_lossy().to_string();
    let qwen_ctx = qwen_asr::context::QwenCtx::load(&dir_str)
        .ok_or_else(|| EngineError::LaunchFailed(
            format!("Failed to load Qwen3-ASR from {}", model_dir.display())
        ))?;
    log::info!("Qwen3-ASR loaded, optimizations: {:?}", qwen_asr::optimization_flags());
    Ok(QwenContext {
        ctx: qwen_ctx,
        model_id: model_id.to_string(),
    })
}

/// Transcribe an audio file using a loaded QwenContext.
pub fn transcribe(ctx: &mut QwenContext, audio_path: &Path, language: &str) -> Result<String, EngineError> {
    // Set forced language if not auto
    if language != "auto" {
        if let Some(lang_name) = lang_code_to_name(language) {
            let _ = ctx.ctx.set_force_language(lang_name);
        }
    } else {
        let _ = ctx.ctx.set_force_language("");
    }

    let audio = read_wav_f32(audio_path)?;

    let text = qwen_asr::transcribe::transcribe_audio(&mut ctx.ctx, &audio)
        .ok_or_else(|| EngineError::LaunchFailed("Qwen3-ASR transcription failed".into()))?;

    log::debug!(
        "Qwen3-ASR: {:.0}ms total, {:.0}ms encode, {:.0}ms decode, {} tokens",
        ctx.ctx.perf_total_ms,
        ctx.ctx.perf_encode_ms,
        ctx.ctx.perf_decode_ms,
        ctx.ctx.perf_text_tokens,
    );

    Ok(text.trim().to_string())
}

// -- Engine (catalogue) --

pub struct QwenEngine;

fn storage_dir() -> String {
    jona_types::models_dir().join("qwen-asr").to_string_lossy().to_string()
}

impl ASREngine for QwenEngine {
    fn engine_id(&self) -> &str { "qwen-asr" }
    fn display_name(&self) -> &str { "Qwen3-ASR" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "qwen-asr:0.6b".into(),
                engine_id: "qwen-asr".into(),
                label: "Qwen3 ASR".into(),
                filename: "0.6b".into(),
                url: String::new(),
                size: 1_880_000_000 + 2_780_000 + 1_670_000,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "model.safetensors".into(),
                            url: "https://huggingface.co/Qwen/Qwen3-ASR-0.6B/resolve/main/model.safetensors".into(),
                            size: 1_880_000_000,
                        },
                        DownloadFile {
                            filename: "vocab.json".into(),
                            url: "https://huggingface.co/Qwen/Qwen3-ASR-0.6B/resolve/main/vocab.json".into(),
                            size: 2_780_000,
                        },
                        DownloadFile {
                            filename: "merges.txt".into(),
                            url: "https://huggingface.co/Qwen/Qwen3-ASR-0.6B/resolve/main/merges.txt".into(),
                            size: 1_670_000,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                wer: Some(2.0),
                rtf: Some(0.15),
                recommended: false,
                params: Some(0.6),
                ram: Some(2_000_000_000),
                lang_codes: Some(vec![
                    "en".into(), "fr".into(), "zh".into(), "ja".into(), "ko".into(),
                    "de".into(), "es".into(), "pt".into(), "it".into(), "ru".into(),
                    "ar".into(), "tr".into(), "hi".into(), "th".into(), "vi".into(),
                    "id".into(), "ms".into(), "nl".into(), "sv".into(), "da".into(),
                    "fi".into(), "pl".into(), "cs".into(), "ro".into(), "hu".into(),
                    "el".into(), "fa".into(), "fil".into(), "mk".into(),
                ]),
                runtime: Some("accelerate".into()),
                quantization: Some("BF16".into()),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "en".into(), label: "English".into() },
            Language { code: "fr".into(), label: "Fran\u{00e7}ais".into() },
            Language { code: "zh".into(), label: "\u{4e2d}\u{6587}".into() },
            Language { code: "ja".into(), label: "\u{65e5}\u{672c}\u{8a9e}".into() },
            Language { code: "ko".into(), label: "\u{d55c}\u{ad6d}\u{c5b4}".into() },
            Language { code: "de".into(), label: "Deutsch".into() },
            Language { code: "es".into(), label: "Espa\u{00f1}ol".into() },
            Language { code: "pt".into(), label: "Portugu\u{00ea}s".into() },
            Language { code: "it".into(), label: "Italiano".into() },
            Language { code: "ru".into(), label: "\u{0420}\u{0443}\u{0441}\u{0441}\u{043a}\u{0438}\u{0439}".into() },
            Language { code: "ar".into(), label: "\u{0627}\u{0644}\u{0639}\u{0631}\u{0628}\u{064a}\u{0629}".into() },
            Language { code: "tr".into(), label: "T\u{00fc}rk\u{00e7}e".into() },
            Language { code: "hi".into(), label: "\u{0939}\u{093f}\u{0928}\u{094d}\u{0926}\u{0940}".into() },
            Language { code: "th".into(), label: "\u{0e44}\u{0e17}\u{0e22}".into() },
            Language { code: "vi".into(), label: "Ti\u{1ebf}ng Vi\u{1ec7}t".into() },
            Language { code: "id".into(), label: "Bahasa Indonesia".into() },
            Language { code: "ms".into(), label: "Bahasa Melayu".into() },
            Language { code: "nl".into(), label: "Nederlands".into() },
            Language { code: "sv".into(), label: "Svenska".into() },
            Language { code: "da".into(), label: "Dansk".into() },
            Language { code: "fi".into(), label: "Suomi".into() },
            Language { code: "pl".into(), label: "Polski".into() },
            Language { code: "cs".into(), label: "\u{010c}e\u{0161}tina".into() },
            Language { code: "ro".into(), label: "Rom\u{00e2}n\u{0103}".into() },
            Language { code: "hu".into(), label: "Magyar".into() },
            Language { code: "el".into(), label: "\u{0395}\u{03bb}\u{03bb}\u{03b7}\u{03bd}\u{03b9}\u{03ba}\u{03ac}".into() },
            Language { code: "fa".into(), label: "\u{0641}\u{0627}\u{0631}\u{0633}\u{06cc}".into() },
            Language { code: "fil".into(), label: "Filipino".into() },
            Language { code: "mk".into(), label: "\u{041c}\u{0430}\u{043a}\u{0435}\u{0434}\u{043e}\u{043d}\u{0441}\u{043a}\u{0438}".into() },
        ]
    }

    fn description(&self) -> &str {
        "Alibaba Qwen3-ASR encoder-decoder LLM. 30 languages, Apple Accelerate (AMX) acceleration."
    }
}
