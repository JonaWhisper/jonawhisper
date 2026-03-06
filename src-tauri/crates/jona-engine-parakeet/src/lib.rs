use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineError, HasModelId, Language,
};
use ort::session::Session;
use ort::value::Tensor;
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

// -- Constants --

const LSTM_DIM: usize = 640;
const NUM_LSTM_LAYERS: usize = 2;
const NUM_DURATIONS: usize = 5;
const MAX_TOKENS_PER_STEP: usize = 10;
const MAX_DECODE_TOKENS: usize = 1024;

// -- Context (cached model state) --

/// Cached Parakeet inference context: encoder + decoder ONNX sessions + vocabulary.
pub struct ParakeetContext {
    encoder: Session,
    decoder_joint: Session,
    vocab: Vec<String>,
    vocab_size: usize,
    pub model_id: String,
}

impl HasModelId for ParakeetContext {
    fn model_id(&self) -> &str {
        &self.model_id
    }
}

// -- Loading --

/// Load Parakeet encoder + decoder sessions and vocabulary from a model directory.
pub fn load(model_dir: &Path, model_id: &str) -> Result<ParakeetContext, EngineError> {
    let encoder_path = find_file(model_dir, &[
        "encoder-model.int8.onnx",
        "encoder-model.onnx",
        "encoder.onnx",
    ]).ok_or_else(|| EngineError::LaunchFailed(
        format!("Encoder ONNX not found in {}", model_dir.display())
    ))?;

    let decoder_path = find_file(model_dir, &[
        "decoder_joint-model.int8.onnx",
        "decoder_joint-model.onnx",
        "decoder_joint.onnx",
        "decoder-model.int8.onnx",
        "decoder-model.onnx",
    ]).ok_or_else(|| EngineError::LaunchFailed(
        format!("Decoder ONNX not found in {}", model_dir.display())
    ))?;

    let vocab_path = model_dir.join("vocab.txt");
    if !vocab_path.exists() {
        return Err(EngineError::LaunchFailed(format!("Vocab not found: {}", vocab_path.display())));
    }

    let n_threads = (jona_engines::ort_session::inference_threads() / 2).max(1);

    log::info!("Loading Parakeet encoder: {}", encoder_path.display());
    let encoder = jona_engines::ort_session::build_session(n_threads)
        .map_err(EngineError::LaunchFailed)?
        .commit_from_file(&encoder_path)
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to load encoder: {e}")))?;

    log::info!("Loading Parakeet decoder-joint: {}", decoder_path.display());
    let decoder_joint = jona_engines::ort_session::build_session(n_threads)
        .map_err(EngineError::LaunchFailed)?
        .commit_from_file(&decoder_path)
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to load decoder: {e}")))?;

    let vocab_text = std::fs::read_to_string(&vocab_path)
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to read vocab: {e}")))?;
    let vocab = parse_vocab(&vocab_text)
        .map_err(EngineError::LaunchFailed)?;
    let vocab_size = vocab.len();

    log::info!(
        "Parakeet loaded: {} vocab tokens, blank_id={}",
        vocab_size, vocab_size - 1,
    );

    Ok(ParakeetContext {
        encoder,
        decoder_joint,
        vocab,
        vocab_size,
        model_id: model_id.to_string(),
    })
}

// -- Inference --

/// Transcribe an audio file using a loaded ParakeetContext.
pub fn transcribe(ctx: &mut ParakeetContext, audio_path: &Path, _language: &str) -> Result<String, EngineError> {
    let audio = read_wav_f32(audio_path)?;

    // Compute mel spectrogram with Slaney scale + pre-emphasis (Parakeet config)
    let (features, n_frames) = jona_engines::mel::extract_features_with_config(
        &audio,
        &jona_engines::mel::PARAKEET_CONFIG,
    );

    let (enc_out, enc_dim, time_steps) = run_encoder(ctx, &features, n_frames)?;
    let token_ids = tdt_greedy_decode(ctx, &enc_out, enc_dim, time_steps)?;
    let text = decode_tokens(ctx, &token_ids);

    Ok(text.trim().to_string())
}

// -- Encoder --

fn run_encoder(
    ctx: &mut ParakeetContext,
    features: &[f32],
    n_frames: usize,
) -> Result<(Vec<f32>, usize, usize), EngineError> {
    let signal_tensor = Tensor::from_array(([1usize, 128, n_frames], features.to_vec()))
        .map_err(|e| EngineError::LaunchFailed(format!("Signal tensor: {e}")))?;

    let length_tensor = Tensor::from_array(([1usize], vec![n_frames as i64]))
        .map_err(|e| EngineError::LaunchFailed(format!("Length tensor: {e}")))?;

    let outputs = ctx.encoder.run(
        ort::inputs![
            "audio_signal" => signal_tensor,
            "length" => length_tensor,
        ]
    ).map_err(|e| EngineError::LaunchFailed(format!("Encoder inference: {e}")))?;

    let (shape, data) = outputs["outputs"]
        .try_extract_tensor::<f32>()
        .map_err(|e| EngineError::LaunchFailed(format!("Encoder output: {e}")))?;

    let encoder_dim = if shape.len() >= 3 { shape[1] as usize } else { 512 };
    let time_steps = if shape.len() >= 3 { shape[2] as usize } else { data.len() / encoder_dim };

    log::debug!("Parakeet encoder: dim={}, time_steps={}", encoder_dim, time_steps);

    Ok((data.to_vec(), encoder_dim, time_steps))
}

// -- TDT Decoder --

fn tdt_greedy_decode(
    ctx: &mut ParakeetContext,
    enc_out: &[f32],
    enc_dim: usize,
    time_steps: usize,
) -> Result<Vec<usize>, EngineError> {
    let blank_id = ctx.vocab_size - 1;

    let state_size = NUM_LSTM_LAYERS * LSTM_DIM;
    let mut state_h = vec![0.0f32; state_size];
    let mut state_c = vec![0.0f32; state_size];

    let mut last_token = blank_id as i32;
    let mut tokens: Vec<usize> = Vec::new();
    let mut t = 0usize;
    let mut total_emitted = 0usize;

    while t < time_steps && total_emitted < MAX_DECODE_TOKENS {
        let mut emitted_this_step = 0;

        loop {
            let mut frame = vec![0.0f32; enc_dim];
            for d in 0..enc_dim {
                frame[d] = enc_out[d * time_steps + t];
            }

            let enc_frame_tensor = Tensor::from_array(([1usize, enc_dim, 1usize], frame))
                .map_err(|e| EngineError::LaunchFailed(format!("Enc frame tensor: {e}")))?;

            let targets_tensor = Tensor::from_array(([1usize, 1usize], vec![last_token]))
                .map_err(|e| EngineError::LaunchFailed(format!("Targets tensor: {e}")))?;

            let target_len_tensor = Tensor::from_array(([1usize], vec![1i32]))
                .map_err(|e| EngineError::LaunchFailed(format!("Target len tensor: {e}")))?;

            let state_h_tensor = Tensor::from_array(
                ([NUM_LSTM_LAYERS, 1usize, LSTM_DIM], state_h.clone()),
            ).map_err(|e| EngineError::LaunchFailed(format!("State h tensor: {e}")))?;

            let state_c_tensor = Tensor::from_array(
                ([NUM_LSTM_LAYERS, 1usize, LSTM_DIM], state_c.clone()),
            ).map_err(|e| EngineError::LaunchFailed(format!("State c tensor: {e}")))?;

            let outputs = ctx.decoder_joint.run(
                ort::inputs![
                    "encoder_outputs" => enc_frame_tensor,
                    "targets" => targets_tensor,
                    "target_length" => target_len_tensor,
                    "input_states_1" => state_h_tensor,
                    "input_states_2" => state_c_tensor,
                ]
            ).map_err(|e| EngineError::LaunchFailed(format!("Decoder step t={t}: {e}")))?;

            let (_, logits) = outputs["outputs"]
                .try_extract_tensor::<f32>()
                .map_err(|e| EngineError::LaunchFailed(format!("Decoder logits: {e}")))?;

            let vocab_logits = &logits[..ctx.vocab_size];
            let duration_logits = &logits[ctx.vocab_size..ctx.vocab_size + NUM_DURATIONS];

            let token_id = vocab_logits.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(blank_id);

            let duration_step = duration_logits.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0);

            if token_id != blank_id {
                let (_, new_h) = outputs["output_states_1"]
                    .try_extract_tensor::<f32>()
                    .map_err(|e| EngineError::LaunchFailed(format!("State h: {e}")))?;
                state_h = new_h.to_vec();

                let (_, new_c) = outputs["output_states_2"]
                    .try_extract_tensor::<f32>()
                    .map_err(|e| EngineError::LaunchFailed(format!("State c: {e}")))?;
                state_c = new_c.to_vec();

                tokens.push(token_id);
                last_token = token_id as i32;
                total_emitted += 1;
                emitted_this_step += 1;
            }

            if duration_step > 0 {
                t += duration_step;
                break;
            } else if token_id == blank_id || emitted_this_step >= MAX_TOKENS_PER_STEP {
                t += 1;
                break;
            }
        }
    }

    log::debug!("Parakeet TDT: emitted {} tokens over {} frames", total_emitted, time_steps);
    Ok(tokens)
}

// -- Detokenization --

fn decode_tokens(ctx: &ParakeetContext, tokens: &[usize]) -> String {
    let mut text = String::new();

    for &id in tokens {
        if id >= ctx.vocab.len() {
            continue;
        }
        let token = &ctx.vocab[id];

        if (token.starts_with('<') && token.ends_with('>')) && token != "<unk>" {
            continue;
        }

        let replaced = token.replace('\u{2581}', " ");
        text.push_str(&replaced);
    }

    text
}

// -- Vocab parsing --

fn parse_vocab(text: &str) -> Result<Vec<String>, String> {
    let mut entries: Vec<(String, usize)> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(last_space) = line.rfind(' ') {
            let token = &line[..last_space];
            let id_str = &line[last_space + 1..];
            if let Ok(id) = id_str.parse::<usize>() {
                entries.push((token.to_string(), id));
            }
        }
    }

    if entries.is_empty() {
        return Err("Empty or invalid vocab.txt".into());
    }

    entries.sort_by_key(|(_, id)| *id);

    let max_id = entries.last().map(|(_, id)| *id).unwrap_or(0);
    let mut vocab = vec![String::new(); max_id + 1];

    for (token, id) in &entries {
        if *id < vocab.len() {
            vocab[*id].clone_from(token);
        }
    }

    Ok(vocab)
}

// -- Utility --

fn find_file(dir: &Path, candidates: &[&str]) -> Option<std::path::PathBuf> {
    for name in candidates {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

// -- Engine (catalogue) --

pub struct ParakeetEngine;

fn storage_dir() -> String {
    jona_types::models_dir().join("parakeet").to_string_lossy().to_string()
}

impl ASREngine for ParakeetEngine {
    fn engine_id(&self) -> &str { "parakeet" }
    fn display_name(&self) -> &str { "Parakeet" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "parakeet:tdt-0.6b-v3-int8".into(),
                engine_id: "parakeet".into(),
                label: "Parakeet TDT V3".into(),
                quantization: Some("INT8".into()),
                filename: "tdt-0.6b-v3-int8".into(),
                url: String::new(),
                size: 683_574_784 + 19_078_554 + 96_153,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/encoder-model.int8.onnx".into(),
                            size: 683_574_784,
                        },
                        DownloadFile {
                            filename: "decoder_joint-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/decoder_joint-model.int8.onnx".into(),
                            size: 19_078_554,
                        },
                        DownloadFile {
                            filename: "vocab.txt".into(),
                            url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/vocab.txt".into(),
                            size: 96_153,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                wer: Some(1.5),
                rtf: Some(0.10),
                recommended: false,
                params: Some(0.6),
                ram: Some(750_000_000),
                lang_codes: Some(vec![
                    "en".into(), "fr".into(), "de".into(), "es".into(), "it".into(),
                    "pt".into(), "nl".into(), "pl".into(), "ru".into(), "uk".into(),
                    "sv".into(), "da".into(), "fi".into(), "ro".into(), "hu".into(),
                    "cs".into(), "sk".into(), "bg".into(), "hr".into(), "sl".into(),
                    "el".into(), "lt".into(), "lv".into(), "et".into(), "mt".into(),
                ]),
                runtime: Some("ort".into()),
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
            Language { code: "pl".into(), label: "Polski".into() },
            Language { code: "ru".into(), label: "\u{0420}\u{0443}\u{0441}\u{0441}\u{043a}\u{0438}\u{0439}".into() },
            Language { code: "uk".into(), label: "\u{0423}\u{043a}\u{0440}\u{0430}\u{0457}\u{043d}\u{0441}\u{044c}\u{043a}\u{0430}".into() },
            Language { code: "sv".into(), label: "Svenska".into() },
            Language { code: "da".into(), label: "Dansk".into() },
            Language { code: "fi".into(), label: "Suomi".into() },
            Language { code: "ro".into(), label: "Rom\u{00e2}n\u{0103}".into() },
            Language { code: "hu".into(), label: "Magyar".into() },
            Language { code: "cs".into(), label: "\u{010c}e\u{0161}tina".into() },
            Language { code: "sk".into(), label: "Sloven\u{010d}ina".into() },
            Language { code: "bg".into(), label: "\u{0411}\u{044a}\u{043b}\u{0433}\u{0430}\u{0440}\u{0441}\u{043a}\u{0438}".into() },
            Language { code: "hr".into(), label: "Hrvatski".into() },
            Language { code: "sl".into(), label: "Sloven\u{0161}\u{010d}ina".into() },
            Language { code: "el".into(), label: "\u{0395}\u{03bb}\u{03bb}\u{03b7}\u{03bd}\u{03b9}\u{03ba}\u{03ac}".into() },
            Language { code: "lt".into(), label: "Lietuvi\u{0173}".into() },
            Language { code: "lv".into(), label: "Latvie\u{0161}u".into() },
            Language { code: "et".into(), label: "Eesti".into() },
            Language { code: "mt".into(), label: "Malti".into() },
        ]
    }

    fn description(&self) -> &str {
        "NVIDIA Parakeet TDT transducer ASR. 25 European languages with auto-detection, excellent quality."
    }
}
