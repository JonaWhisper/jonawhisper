//! NVIDIA Parakeet-TDT ASR inference via ONNX Runtime (ort).
//!
//! Pipeline: WAV → mel spectrogram (Slaney, pre-emphasis) → encoder → TDT decoder → text.
//! Vendored from parakeet-rs TDT subset, adapted for JonaWhisper with CoreML EP.

use crate::engines::{ASRModel, EngineError};
use crate::state::AppState;
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;

/// LSTM hidden dimension for Parakeet-TDT 0.6B.
const LSTM_DIM: usize = 640;
/// Number of LSTM layers.
const NUM_LSTM_LAYERS: usize = 2;
/// Number of duration head outputs (skip 0-4 frames).
const NUM_DURATIONS: usize = 5;
/// Maximum tokens per time step before forcing frame advance.
const MAX_TOKENS_PER_STEP: usize = 10;
/// Maximum total decode tokens.
const MAX_DECODE_TOKENS: usize = 1024;

/// Cached Parakeet inference context: encoder + decoder ONNX sessions + vocabulary.
pub struct ParakeetContext {
    encoder: Session,
    decoder_joint: Session,
    vocab: Vec<String>,
    vocab_size: usize,
    pub model_id: String,
}

impl crate::state::HasModelId for ParakeetContext {
    fn model_id(&self) -> &str {
        &self.model_id
    }
}

impl ParakeetContext {
    pub fn load(model_dir: &Path, model_id: &str) -> Result<Self, String> {
        // Find encoder file
        let encoder_path = find_file(model_dir, &[
            "encoder-model.int8.onnx",
            "encoder-model.onnx",
            "encoder.onnx",
        ]).ok_or_else(|| format!("Encoder ONNX not found in {}", model_dir.display()))?;

        // Find decoder-joint file
        let decoder_path = find_file(model_dir, &[
            "decoder_joint-model.int8.onnx",
            "decoder_joint-model.onnx",
            "decoder_joint.onnx",
            "decoder-model.int8.onnx",
            "decoder-model.onnx",
        ]).ok_or_else(|| format!("Decoder ONNX not found in {}", model_dir.display()))?;

        let vocab_path = model_dir.join("vocab.txt");
        if !vocab_path.exists() {
            return Err(format!("Vocab not found: {}", vocab_path.display()));
        }

        let n_threads = (crate::engines::ort_session::inference_threads() / 2).max(1);

        log::info!("Loading Parakeet encoder: {}", encoder_path.display());
        let encoder = crate::engines::ort_session::build_session(n_threads)?
            .commit_from_file(&encoder_path)
            .map_err(|e| format!("Failed to load encoder: {e}"))?;

        log::info!("Loading Parakeet decoder-joint: {}", decoder_path.display());
        let decoder_joint = crate::engines::ort_session::build_session(n_threads)?
            .commit_from_file(&decoder_path)
            .map_err(|e| format!("Failed to load decoder: {e}"))?;

        // Load vocabulary
        let vocab_text = std::fs::read_to_string(&vocab_path)
            .map_err(|e| format!("Failed to read vocab: {e}"))?;
        let vocab = parse_vocab(&vocab_text)?;
        let vocab_size = vocab.len();

        log::info!(
            "Parakeet loaded: {} vocab tokens, blank_id={}",
            vocab_size, vocab_size - 1,
        );

        Ok(Self {
            encoder,
            decoder_joint,
            vocab,
            vocab_size,
            model_id: model_id.to_string(),
        })
    }
}

/// Transcribe an audio file using Parakeet-TDT ASR.
pub fn transcribe(
    state: &AppState,
    model: &ASRModel,
    audio_path: &Path,
    _language: &str,
) -> Result<String, EngineError> {
    let model_dir = model.local_path();
    if !model_dir.is_dir() {
        return Err(EngineError::ModelNotFound(model_dir.display().to_string()));
    }

    let model_id = model.id.clone();
    let mut ctx_guard = state.inference.asr.parakeet.get_or_load(&model_id, || {
        log::info!("Loading Parakeet model: {}", model_id);
        ParakeetContext::load(&model_dir, &model_id).map_err(EngineError::LaunchFailed)
    })?;
    let ctx = ctx_guard.as_mut().unwrap();

    // Read WAV audio
    let audio = crate::audio::read_wav_f32(audio_path)?;

    // Compute mel spectrogram with Slaney scale + pre-emphasis
    let (features, n_frames) = super::mel::extract_features_with_config(
        &audio,
        &super::mel::PARAKEET_CONFIG,
    );

    // Run encoder
    let (enc_out, enc_dim, time_steps) = run_encoder(ctx, &features, n_frames)?;

    // Run TDT greedy decode
    let token_ids = tdt_greedy_decode(ctx, &enc_out, enc_dim, time_steps)?;

    // Detokenize
    let text = decode_tokens(ctx, &token_ids);

    Ok(text.trim().to_string())
}

/// Run the encoder ONNX model.
/// Returns (flattened encoder output, encoder_dim, time_steps).
fn run_encoder(
    ctx: &mut ParakeetContext,
    features: &[f32],
    n_frames: usize,
) -> Result<(Vec<f32>, usize, usize), EngineError> {
    // Input: audio_signal [1, 128, n_frames]
    let signal_tensor = Tensor::from_array(([1usize, 128, n_frames], features.to_vec()))
        .map_err(|e| EngineError::LaunchFailed(format!("Signal tensor: {e}")))?;

    // Input: length [1]
    let length_tensor = Tensor::from_array(([1usize], vec![n_frames as i64]))
        .map_err(|e| EngineError::LaunchFailed(format!("Length tensor: {e}")))?;

    let outputs = ctx.encoder.run(
        ort::inputs![
            "audio_signal" => signal_tensor,
            "length" => length_tensor,
        ]
    ).map_err(|e| EngineError::LaunchFailed(format!("Encoder inference: {e}")))?;

    // Output: "outputs" [1, encoder_dim, time_steps]
    let (shape, data) = outputs["outputs"]
        .try_extract_tensor::<f32>()
        .map_err(|e| EngineError::LaunchFailed(format!("Encoder output: {e}")))?;

    let encoder_dim = if shape.len() >= 3 { shape[1] as usize } else { 512 };
    let time_steps = if shape.len() >= 3 { shape[2] as usize } else { data.len() / encoder_dim };

    log::debug!("Parakeet encoder: dim={}, time_steps={}", encoder_dim, time_steps);

    Ok((data.to_vec(), encoder_dim, time_steps))
}

/// TDT greedy decode: frame-by-frame through decoder_joint with LSTM states.
fn tdt_greedy_decode(
    ctx: &mut ParakeetContext,
    enc_out: &[f32],
    enc_dim: usize,
    time_steps: usize,
) -> Result<Vec<usize>, EngineError> {
    let blank_id = ctx.vocab_size - 1;

    // LSTM states: [NUM_LSTM_LAYERS, 1, LSTM_DIM]
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
            // Extract single encoder frame: enc_out[0, :, t] → [1, enc_dim, 1]
            let mut frame = vec![0.0f32; enc_dim];
            for d in 0..enc_dim {
                // enc_out is [1, enc_dim, time_steps] row-major
                frame[d] = enc_out[d * time_steps + t];
            }

            // Run decoder_joint
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

            // Extract logits: vocab_size + NUM_DURATIONS values
            let (_, logits) = outputs["outputs"]
                .try_extract_tensor::<f32>()
                .map_err(|e| EngineError::LaunchFailed(format!("Decoder logits: {e}")))?;

            let vocab_logits = &logits[..ctx.vocab_size];
            let duration_logits = &logits[ctx.vocab_size..ctx.vocab_size + NUM_DURATIONS];

            // Argmax for token
            let token_id = vocab_logits.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(blank_id);

            // Argmax for duration
            let duration_step = duration_logits.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0);

            if token_id != blank_id {
                // Update LSTM states from output
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

            // Frame advance
            if duration_step > 0 {
                t += duration_step;
                break;
            } else if token_id == blank_id || emitted_this_step >= MAX_TOKENS_PER_STEP {
                t += 1;
                break;
            }
            // Continue emitting at same frame
        }
    }

    log::debug!("Parakeet TDT: emitted {} tokens over {} frames", total_emitted, time_steps);
    Ok(tokens)
}

/// Convert token IDs to text, handling SentencePiece ▁ markers.
fn decode_tokens(ctx: &ParakeetContext, tokens: &[usize]) -> String {
    let mut text = String::new();

    for &id in tokens {
        if id >= ctx.vocab.len() {
            continue;
        }
        let token = &ctx.vocab[id];

        // Skip special tokens
        if (token.starts_with('<') && token.ends_with('>')) && token != "<unk>" {
            continue;
        }

        // SentencePiece: ▁ = word boundary (space)
        let replaced = token.replace('\u{2581}', " ");
        text.push_str(&replaced);
    }

    text
}

/// Parse vocab.txt: "token_text token_id" per line.
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

/// Find the first existing file from a list of candidates in a directory.
fn find_file(dir: &Path, candidates: &[&str]) -> Option<std::path::PathBuf> {
    for name in candidates {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}
