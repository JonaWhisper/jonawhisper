use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineError, HasModelId, Language,
};
use ort::session::Session;
use ort::value::Tensor;
use std::collections::HashMap;
use std::path::Path;

// -- Audio utility (inline, avoids dependency on main crate) --

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

const MAX_DECODE_TOKENS: usize = 512;
const DEFAULT_NUM_LAYERS: usize = 4;

// -- Context (cached model state) --

/// Cached Canary inference context: encoder + decoder ONNX sessions + vocabulary.
pub struct CanaryContext {
    encoder: Session,
    decoder: Session,
    vocab: Vec<String>,
    token_to_id: HashMap<String, i64>,
    is_sentencepiece: bool,
    pub model_id: String,
}

impl HasModelId for CanaryContext {
    fn model_id(&self) -> &str {
        &self.model_id
    }
}

impl CanaryContext {
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

// -- Loading --

/// Load Canary encoder + decoder sessions and vocabulary from a model directory.
pub fn load(model_dir: &Path, model_id: &str) -> Result<CanaryContext, EngineError> {
    let encoder_path = model_dir.join("encoder-model.int8.onnx");
    let decoder_path = model_dir.join("decoder-model.int8.onnx");
    let vocab_path = model_dir.join("vocab.txt");

    if !encoder_path.exists() {
        return Err(EngineError::LaunchFailed(format!("Encoder not found: {}", encoder_path.display())));
    }
    if !decoder_path.exists() {
        return Err(EngineError::LaunchFailed(format!("Decoder not found: {}", decoder_path.display())));
    }
    if !vocab_path.exists() {
        return Err(EngineError::LaunchFailed(format!("Vocab not found: {}", vocab_path.display())));
    }

    let n_threads = (jona_engines::ort_session::inference_threads() / 2).max(1);

    log::info!("Loading Canary encoder: {}", encoder_path.display());
    let encoder = jona_engines::ort_session::build_session(n_threads)
        .map_err(EngineError::LaunchFailed)?
        .commit_from_file(&encoder_path)
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to load encoder: {e}")))?;

    log::info!("Loading Canary decoder: {}", decoder_path.display());
    let decoder = jona_engines::ort_session::build_session(n_threads)
        .map_err(EngineError::LaunchFailed)?
        .commit_from_file(&decoder_path)
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to load decoder: {e}")))?;

    let vocab_text = std::fs::read_to_string(&vocab_path)
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to read vocab: {e}")))?;
    let (vocab, token_to_id) = parse_vocab(&vocab_text)
        .map_err(EngineError::LaunchFailed)?;

    let is_sentencepiece = vocab.iter().any(|t| t.contains('\u{2581}'));

    log::info!(
        "Canary loaded: {} vocab tokens, sentencepiece={}",
        vocab.len(), is_sentencepiece
    );

    Ok(CanaryContext {
        encoder,
        decoder,
        vocab,
        token_to_id,
        is_sentencepiece,
        model_id: model_id.to_string(),
    })
}

// -- Inference --

/// Transcribe an audio file using a loaded CanaryContext.
pub fn transcribe(ctx: &mut CanaryContext, audio_path: &Path, language: &str) -> Result<String, EngineError> {
    let audio = read_wav_f32(audio_path)?;

    // Compute mel spectrogram (Canary config: HTK mel scale)
    let (features, n_frames) = jona_engines::mel::extract_features(&audio);

    // Run encoder
    let enc_result = run_encoder(ctx, &features, n_frames)?;

    // Resolve language for prompt
    let lang = if language == "auto" { "en" } else { language };

    // Build prompt tokens: [BOS, target_lang, source_lang, pnc_token]
    let mut prompt_tokens: Vec<i64> = Vec::with_capacity(4);
    prompt_tokens.push(ctx.bos_id());
    if let Some(id) = ctx.lang_token_id(lang) {
        prompt_tokens.push(id);
    }
    if let Some(id) = ctx.lang_token_id(lang) {
        prompt_tokens.push(id);
    }
    if let Some(id) = ctx.pnc_token_id(true) {
        prompt_tokens.push(id);
    }

    let output_tokens = run_decoder(ctx, &prompt_tokens, &enc_result)?;
    let text = decode_tokens(ctx, &output_tokens);

    Ok(text.trim().to_string())
}

// -- Encoder --

struct EncoderResult {
    embeddings: Vec<f32>,
    emb_shape: [usize; 3],
    mask: Vec<i64>,
    mask_len: usize,
    hidden_dim: usize,
}

fn run_encoder(
    ctx: &mut CanaryContext,
    features: &[f32],
    n_frames: usize,
) -> Result<EncoderResult, EngineError> {
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

    let (emb_shape_raw, emb_data) = outputs["encoder_embeddings"]
        .try_extract_tensor::<f32>()
        .map_err(|e| EngineError::LaunchFailed(format!("Encoder embeddings: {e}")))?;
    let hidden_dim = if emb_shape_raw.len() >= 3 { emb_shape_raw[2] as usize } else { 256 };
    let enc_seq_len = if emb_shape_raw.len() >= 2 { emb_shape_raw[1] as usize } else { emb_data.len() / hidden_dim };
    let emb_shape = [1, enc_seq_len, hidden_dim];

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

// -- Decoder --

fn run_decoder(
    ctx: &mut CanaryContext,
    prompt_tokens: &[i64],
    enc: &EncoderResult,
) -> Result<Vec<i64>, EngineError> {
    let eos_id = ctx.eos_id();
    let mut output_tokens: Vec<i64> = Vec::new();

    let num_layers = DEFAULT_NUM_LAYERS;
    let mut cache_data: Vec<f32> = Vec::new();
    let mut cache_shape: [usize; 4] = [num_layers, 1, 0, enc.hidden_dim];

    let mut input_ids = prompt_tokens.to_vec();

    for step in 0..MAX_DECODE_TOKENS {
        let seq_len = input_ids.len();

        let ids_tensor = Tensor::from_array(([1usize, seq_len], input_ids.clone()))
            .map_err(|e| EngineError::LaunchFailed(format!("Decoder ids: {e}")))?;

        let enc_tensor = Tensor::from_array((enc.emb_shape, enc.embeddings.clone()))
            .map_err(|e| EngineError::LaunchFailed(format!("Enc tensor: {e}")))?;

        let mask_tensor = Tensor::from_array(([1usize, enc.mask_len], enc.mask.clone()))
            .map_err(|e| EngineError::LaunchFailed(format!("Mask tensor: {e}")))?;

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

        let (logits_shape, logits_data) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| EngineError::LaunchFailed(format!("Logits: {e}")))?;
        let vocab_size = if logits_shape.len() >= 3 { logits_shape[2] as usize } else { logits_data.len() };
        let out_seq_len = if logits_shape.len() >= 2 { logits_shape[1] as usize } else { 1 };

        let last_offset = (out_seq_len - 1) * vocab_size;
        let last_logits = &logits_data[last_offset..last_offset + vocab_size];

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

        input_ids = vec![next_token];

        if step == 0 {
            log::debug!("Canary decoder: first step done, prompt={} tokens", prompt_tokens.len());
        }
    }

    Ok(output_tokens)
}

// -- Detokenization --

fn decode_tokens(ctx: &CanaryContext, tokens: &[i64]) -> String {
    let mut text = String::new();

    for &id in tokens {
        let idx = id as usize;
        if idx >= ctx.vocab.len() {
            continue;
        }

        let token = &ctx.vocab[idx];

        if token.starts_with("<|") || token.starts_with("</") || token == "<unk>" || token == "<pad>" {
            continue;
        }

        if ctx.is_sentencepiece {
            let replaced = token.replace('\u{2581}', " ");
            text.push_str(&replaced);
        } else {
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

// -- Vocab parsing --

fn parse_vocab(text: &str) -> Result<(Vec<String>, HashMap<String, i64>), String> {
    let mut entries: Vec<(String, i64)> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

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

// -- Engine (catalogue) --

pub struct CanaryEngine;

fn storage_dir() -> String {
    jona_types::models_dir().join("canary").to_string_lossy().to_string()
}

impl ASREngine for CanaryEngine {
    fn engine_id(&self) -> &str { "canary" }
    fn display_name(&self) -> &str { "Canary" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "canary:180m-flash-int8".into(),
                engine_id: "canary".into(),
                label: "Canary Flash".into(),
                quantization: Some("INT8".into()),
                filename: "180m-flash-int8".into(),
                url: String::new(),
                size: 213_284_662,
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/canary-180m-flash-onnx/resolve/main/encoder-model.int8.onnx".into(),
                            size: 133_710_896,
                        },
                        DownloadFile {
                            filename: "decoder-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/canary-180m-flash-onnx/resolve/main/decoder-model.int8.onnx".into(),
                            size: 79_520_211,
                        },
                        DownloadFile {
                            filename: "vocab.txt".into(),
                            url: "https://huggingface.co/istupakov/canary-180m-flash-onnx/resolve/main/vocab.txt".into(),
                            size: 53_555,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                wer: Some(1.87),
                rtf: Some(0.15),
                recommended: false,
                params: Some(0.182),
                ram: Some(300_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "de".into(), "es".into()]),
                runtime: Some("ort".into()),
                ..Default::default()
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "fr".into(), label: "Fran\u{00e7}ais".into() },
            Language { code: "en".into(), label: "English".into() },
            Language { code: "de".into(), label: "Deutsch".into() },
            Language { code: "es".into(), label: "Espa\u{00f1}ol".into() },
        ]
    }

    fn description(&self) -> &str {
        "NVIDIA Canary encoder-decoder ASR. Ultra-light (182M params), beats Whisper Medium quality."
    }
}
