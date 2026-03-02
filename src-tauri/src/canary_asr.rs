//! NVIDIA Canary ASR inference via ONNX Runtime (ort).
//!
//! Pipeline: WAV → mel spectrogram → encoder → autoregressive decoder → text.
//! Based on canary-rs (github.com/mmende/canary-rs) patterns, adapted for WhisperDictate.

use crate::engines::{ASRModel, EngineError};
use crate::state::AppState;
use ort::session::Session;
use ort::value::Tensor;
use std::collections::HashMap;
use std::path::Path;

const MAX_DECODE_TOKENS: usize = 512;
/// Default number of decoder layers for Canary 180M Flash.
const DEFAULT_NUM_LAYERS: usize = 4;

/// Cached Canary inference context: encoder + decoder ONNX sessions + vocabulary.
pub struct CanaryContext {
    encoder: Session,
    decoder: Session,
    /// id-to-token lookup (sorted by ID)
    vocab: Vec<String>,
    /// token-to-id lookup
    token_to_id: HashMap<String, i64>,
    /// Whether vocab uses SentencePiece ▁ markers
    is_sentencepiece: bool,
    /// Model ID for cache invalidation
    pub model_id: String,
}

impl crate::state::HasModelId for CanaryContext {
    fn model_id(&self) -> &str {
        &self.model_id
    }
}

impl CanaryContext {
    /// Load encoder + decoder sessions and vocabulary from a model directory.
    pub fn load(model_dir: &Path, model_id: &str) -> Result<Self, String> {
        let encoder_path = model_dir.join("encoder-model.int8.onnx");
        let decoder_path = model_dir.join("decoder-model.int8.onnx");
        let vocab_path = model_dir.join("vocab.txt");

        if !encoder_path.exists() {
            return Err(format!("Encoder not found: {}", encoder_path.display()));
        }
        if !decoder_path.exists() {
            return Err(format!("Decoder not found: {}", decoder_path.display()));
        }
        if !vocab_path.exists() {
            return Err(format!("Vocab not found: {}", vocab_path.display()));
        }

        let n_threads = std::thread::available_parallelism()
            .map(|p| (p.get() / 2).max(1))
            .unwrap_or(4);

        log::info!("Loading Canary encoder: {}", encoder_path.display());
        let encoder = crate::ort_session::build_session(n_threads)?
            .commit_from_file(&encoder_path)
            .map_err(|e| format!("Failed to load encoder: {e}"))?;

        log::info!("Loading Canary decoder: {}", decoder_path.display());
        let decoder = crate::ort_session::build_session(n_threads)?
            .commit_from_file(&decoder_path)
            .map_err(|e| format!("Failed to load decoder: {e}"))?;

        // Load vocabulary
        let vocab_text = std::fs::read_to_string(&vocab_path)
            .map_err(|e| format!("Failed to read vocab: {e}"))?;
        let (vocab, token_to_id) = parse_vocab(&vocab_text)?;

        let is_sentencepiece = vocab.iter().any(|t| t.contains('\u{2581}'));

        log::info!(
            "Canary loaded: {} vocab tokens, sentencepiece={}",
            vocab.len(), is_sentencepiece
        );

        Ok(Self {
            encoder,
            decoder,
            vocab,
            token_to_id,
            is_sentencepiece,
            model_id: model_id.to_string(),
        })
    }

    fn token_id(&self, token: &str) -> Option<i64> {
        self.token_to_id.get(token).copied()
    }

    fn bos_id(&self) -> i64 {
        self.token_id("<|startoftranscript|>")
            .or_else(|| self.token_id("<s>"))
            .unwrap_or(0)
    }

    fn eos_id(&self) -> i64 {
        self.token_id("<|endoftext|>")
            .or_else(|| self.token_id("</s>"))
            .unwrap_or(1)
    }

    fn lang_token_id(&self, lang: &str) -> Option<i64> {
        self.token_id(&format!("<|{lang}|>"))
    }

    fn pnc_token_id(&self, use_pnc: bool) -> Option<i64> {
        if use_pnc {
            self.token_id("<|pnc|>")
        } else {
            self.token_id("<|nopnc|>")
        }
    }
}

/// Transcribe an audio file using Canary ASR.
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

    let model_id = model.id.clone();
    let mut ctx_guard = state.inference.canary.get_or_load(&model_id, || {
        log::info!("Loading Canary model: {}", model_id);
        CanaryContext::load(&model_dir, &model_id).map_err(EngineError::LaunchFailed)
    })?;
    let ctx = ctx_guard.as_mut().unwrap();

    // Read WAV audio
    let audio = crate::engines::whisper::read_wav_f32(audio_path)?;

    // Compute mel spectrogram
    let (features, n_frames) = crate::mel_features::extract_features(&audio);

    // Run encoder
    let enc_result = run_encoder(ctx, &features, n_frames)?;

    // Resolve language for prompt
    let lang = if language == "auto" { "en" } else { language };

    // Build prompt tokens: [BOS, target_lang, source_lang, pnc_token]
    let mut prompt_tokens: Vec<i64> = Vec::with_capacity(4);
    prompt_tokens.push(ctx.bos_id());
    if let Some(id) = ctx.lang_token_id(lang) {
        prompt_tokens.push(id); // target language
    }
    if let Some(id) = ctx.lang_token_id(lang) {
        prompt_tokens.push(id); // source language (same for ASR)
    }
    if let Some(id) = ctx.pnc_token_id(true) {
        prompt_tokens.push(id); // punctuation & capitalization
    }

    // Run autoregressive decoder
    let output_tokens = run_decoder(ctx, &prompt_tokens, &enc_result)?;

    // Detokenize
    let text = decode_tokens(ctx, &output_tokens);

    Ok(text.trim().to_string())
}

/// Encoder output data passed to the decoder.
struct EncoderResult {
    /// Flattened encoder embeddings [1, enc_seq_len, hidden_dim]
    embeddings: Vec<f32>,
    /// Shape: [1, enc_seq_len, hidden_dim]
    emb_shape: [usize; 3],
    /// Encoder mask [1, enc_seq_len]
    mask: Vec<i64>,
    /// enc_seq_len
    mask_len: usize,
    /// hidden_dim (for decoder cache init)
    hidden_dim: usize,
}

/// Run the encoder on mel features.
fn run_encoder(
    ctx: &mut CanaryContext,
    features: &[f32],
    n_frames: usize,
) -> Result<EncoderResult, EngineError> {
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

    // Extract encoder_embeddings [1, enc_seq_len, hidden_dim]
    let (emb_shape_raw, emb_data) = outputs["encoder_embeddings"]
        .try_extract_tensor::<f32>()
        .map_err(|e| EngineError::LaunchFailed(format!("Encoder embeddings: {e}")))?;
    let hidden_dim = if emb_shape_raw.len() >= 3 { emb_shape_raw[2] as usize } else { 256 };
    let enc_seq_len = if emb_shape_raw.len() >= 2 { emb_shape_raw[1] as usize } else { emb_data.len() / hidden_dim };
    let emb_shape = [1, enc_seq_len, hidden_dim];

    // Extract encoder_mask — try i64 first, fall back to f32
    let mask_data: Vec<i64> = match outputs["encoder_mask"].try_extract_tensor::<i64>() {
        Ok((_shape, data)) => data.to_vec(),
        Err(_) => {
            let (_shape, data) = outputs["encoder_mask"]
                .try_extract_tensor::<f32>()
                .map_err(|e| EngineError::LaunchFailed(format!("Encoder mask: {e}")))?;
            data.iter().map(|&v| v as i64).collect()
        }
    };

    Ok(EncoderResult {
        embeddings: emb_data.to_vec(),
        emb_shape,
        mask: mask_data,
        mask_len: enc_seq_len,
        hidden_dim,
    })
}

/// Run autoregressive decoder loop with KV cache.
fn run_decoder(
    ctx: &mut CanaryContext,
    prompt_tokens: &[i64],
    enc: &EncoderResult,
) -> Result<Vec<i64>, EngineError> {
    let eos_id = ctx.eos_id();
    let mut output_tokens: Vec<i64> = Vec::new();

    // Initialize empty KV cache: [num_layers, 1, 0, hidden_dim]
    let num_layers = DEFAULT_NUM_LAYERS;
    let mut cache_data: Vec<f32> = Vec::new();
    let mut cache_shape: [usize; 4] = [num_layers, 1, 0, enc.hidden_dim];

    // First decoder call uses full prompt
    let mut input_ids = prompt_tokens.to_vec();

    for step in 0..MAX_DECODE_TOKENS {
        let seq_len = input_ids.len();

        // Build input_ids tensor [1, seq_len]
        let ids_tensor = Tensor::from_array(([1usize, seq_len], input_ids.clone()))
            .map_err(|e| EngineError::LaunchFailed(format!("Decoder ids: {e}")))?;

        // Encoder embeddings (passed every step)
        let enc_tensor = Tensor::from_array((enc.emb_shape, enc.embeddings.clone()))
            .map_err(|e| EngineError::LaunchFailed(format!("Enc tensor: {e}")))?;

        // Encoder mask [1, enc_seq_len]
        let mask_tensor = Tensor::from_array(([1usize, enc.mask_len], enc.mask.clone()))
            .map_err(|e| EngineError::LaunchFailed(format!("Mask tensor: {e}")))?;

        // KV cache tensor [num_layers, 1, cache_seq_len, hidden_dim]
        let cache_tensor = Tensor::from_array((cache_shape, cache_data.clone()))
            .map_err(|e| EngineError::LaunchFailed(format!("Cache tensor: {e}")))?;

        let outputs = ctx.decoder.run(
            ort::inputs![
                "input_ids" => ids_tensor,
                "encoder_embeddings" => enc_tensor,
                "encoder_mask" => mask_tensor,
                "decoder_mems" => cache_tensor,
            ]
        ).map_err(|e| EngineError::LaunchFailed(format!("Decoder step {step}: {e}")))?;

        // Extract logits [1, seq_len, vocab_size] — take last timestep
        let (logits_shape, logits_data) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| EngineError::LaunchFailed(format!("Logits: {e}")))?;
        let vocab_size = if logits_shape.len() >= 3 { logits_shape[2] as usize } else { logits_data.len() };
        let out_seq_len = if logits_shape.len() >= 2 { logits_shape[1] as usize } else { 1 };

        // Get logits for last timestep
        let last_offset = (out_seq_len - 1) * vocab_size;
        let last_logits = &logits_data[last_offset..last_offset + vocab_size];

        // Greedy argmax
        let next_token = last_logits
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i as i64)
            .unwrap_or(eos_id);

        if next_token == eos_id {
            break;
        }

        output_tokens.push(next_token);

        // Update KV cache from decoder_hidden_states [num_layers, 1, total_seq, hidden]
        let (hidden_shape, hidden_data) = outputs["decoder_hidden_states"]
            .try_extract_tensor::<f32>()
            .map_err(|e| EngineError::LaunchFailed(format!("Hidden states: {e}")))?;
        if hidden_shape.len() >= 4 {
            cache_shape = [
                hidden_shape[0] as usize,
                hidden_shape[1] as usize,
                hidden_shape[2] as usize,
                hidden_shape[3] as usize,
            ];
        }
        cache_data = hidden_data.to_vec();

        // Next step: only the new token
        input_ids = vec![next_token];

        if step == 0 {
            log::debug!("Canary decoder: first step done, prompt={} tokens", prompt_tokens.len());
        }
    }

    Ok(output_tokens)
}

/// Convert token IDs to text, handling SentencePiece and BPE patterns.
fn decode_tokens(ctx: &CanaryContext, tokens: &[i64]) -> String {
    let mut text = String::new();

    for &id in tokens {
        let idx = id as usize;
        if idx >= ctx.vocab.len() {
            continue;
        }

        let token = &ctx.vocab[idx];

        // Skip special tokens
        if token.starts_with("<|") || token.starts_with("</") || token == "<unk>" || token == "<pad>" {
            continue;
        }

        if ctx.is_sentencepiece {
            // SentencePiece: ▁ = word boundary (space)
            let replaced = token.replace('\u{2581}', " ");
            text.push_str(&replaced);
        } else {
            // BPE: ## prefix = continuation
            if let Some(stripped) = token.strip_prefix("##") {
                text.push_str(stripped);
            } else if !text.is_empty() {
                text.push(' ');
                text.push_str(token);
            } else {
                text.push_str(token);
            }
        }
    }

    text
}

/// Parse vocab.txt: "token_text token_id" per line.
fn parse_vocab(text: &str) -> Result<(Vec<String>, HashMap<String, i64>), String> {
    let mut entries: Vec<(String, i64)> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Split by last space to get (token_text, id)
        if let Some(last_space) = line.rfind(' ') {
            let token = &line[..last_space];
            let id_str = &line[last_space + 1..];
            if let Ok(id) = id_str.parse::<i64>() {
                entries.push((token.to_string(), id));
            }
        }
    }

    if entries.is_empty() {
        return Err("Empty or invalid vocab.txt".into());
    }

    // Sort by ID and build lookup vectors
    entries.sort_by_key(|(_, id)| *id);

    let max_id = entries.last().map(|(_, id)| *id).unwrap_or(0) as usize;
    let mut vocab = vec![String::new(); max_id + 1];
    let mut token_to_id = HashMap::with_capacity(entries.len());

    for (token, id) in &entries {
        let idx = *id as usize;
        if idx < vocab.len() {
            vocab[idx].clone_from(token);
        }
        token_to_id.insert(token.clone(), *id);
    }

    Ok((vocab, token_to_id))
}
