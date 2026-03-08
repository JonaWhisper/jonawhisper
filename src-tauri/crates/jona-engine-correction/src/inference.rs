use std::path::Path;

use ort::session::Session;
use ort::value::Tensor as OrtTensor;
use serde::Deserialize;
use tokenizers::Tokenizer;

/// Subset of T5 config.json we need.
#[derive(Deserialize)]
struct T5Config {
    decoder_start_token_id: Option<u32>,
    eos_token_id: Option<serde_json::Value>, // can be int or array
    vocab_size: usize,
}

/// Cached T5 context: encoder + decoder ONNX sessions + tokenizer.
pub struct T5Context {
    encoder: Session,
    decoder: Session,
    tokenizer: Tokenizer,
    decoder_start_id: i64,
    eos_id: i64,
}

impl T5Context {
    /// Load a T5 correction model from a directory containing
    /// encoder_model.onnx, decoder_model.onnx, config.json, tokenizer.json.
    pub fn load(model_dir: &Path) -> Result<Self, String> {
        let config_data = std::fs::read_to_string(model_dir.join("config.json"))
            .map_err(|e| format!("Failed to read config.json: {e}"))?;
        let config: T5Config = serde_json::from_str(&config_data)
            .map_err(|e| format!("Failed to parse config.json: {e}"))?;

        let eos_id = parse_eos_id(&config);
        let decoder_start_id = config.decoder_start_token_id.unwrap_or(0) as i64;

        let n_threads = jona_engines::ort_session::inference_threads();

        // Prefer INT8 quantized models if available (75% smaller, ~2-3x faster)
        let encoder_path = prefer_int8(model_dir, "encoder_model");
        let decoder_path = prefer_int8(model_dir, "decoder_model");

        if !encoder_path.exists() {
            return Err(format!("Encoder not found: {}", encoder_path.display()));
        }
        if !decoder_path.exists() {
            return Err(format!("Decoder not found: {}", decoder_path.display()));
        }

        log::info!("Loading T5 encoder: {}", encoder_path.display());
        let encoder = jona_engines::ort_session::build_session(n_threads)
            .map_err(|e| format!("Encoder session: {e}"))?
            .commit_from_file(&encoder_path)
            .map_err(|e| format!("Failed to load encoder ONNX: {e}"))?;

        log::info!("Loading T5 decoder: {}", decoder_path.display());
        let decoder = jona_engines::ort_session::build_session(n_threads)
            .map_err(|e| format!("Decoder session: {e}"))?
            .commit_from_file(&decoder_path)
            .map_err(|e| format!("Failed to load decoder ONNX: {e}"))?;

        let tokenizer = Tokenizer::from_file(model_dir.join("tokenizer.json"))
            .map_err(|e| format!("Failed to load tokenizer: {e}"))?;

        log::info!(
            "T5 ONNX loaded: {} (vocab={})",
            model_dir.display(),
            config.vocab_size
        );

        Ok(Self {
            encoder,
            decoder,
            tokenizer,
            decoder_start_id,
            eos_id,
        })
    }
}

/// Return INT8 path if it exists, otherwise FP32.
fn prefer_int8(model_dir: &Path, base_name: &str) -> std::path::PathBuf {
    let int8 = model_dir.join(format!("{base_name}_int8.onnx"));
    if int8.exists() {
        log::info!("Using INT8 model: {}", int8.display());
        return int8;
    }
    model_dir.join(format!("{base_name}.onnx"))
}

fn parse_eos_id(config: &T5Config) -> i64 {
    match &config.eos_token_id {
        Some(serde_json::Value::Number(n)) => n.as_i64().unwrap_or(1),
        Some(serde_json::Value::Array(arr)) => {
            arr.first().and_then(|v| v.as_i64()).unwrap_or(1)
        }
        _ => 1,
    }
}

/// Run T5 correction on a text. Returns the corrected text.
pub fn correct(ctx: &mut T5Context, text: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(text.to_string());
    }

    let input_len = text.len();

    // Tokenize input
    let encoding = ctx
        .tokenizer
        .encode(text, false)
        .map_err(|e| format!("Tokenization failed: {e}"))?;
    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
    let input_token_count = input_ids.len();
    let enc_seq_len = input_ids.len();

    // Run encoder
    let attention_mask: Vec<i64> = vec![1i64; enc_seq_len];

    let ids_tensor = OrtTensor::from_array(([1usize, enc_seq_len], input_ids))
        .map_err(|e| format!("Encoder input: {e}"))?;
    let mask_tensor = OrtTensor::from_array(([1usize, enc_seq_len], attention_mask.clone()))
        .map_err(|e| format!("Encoder mask: {e}"))?;

    let enc_outputs = ctx
        .encoder
        .run(ort::inputs![
            "input_ids" => ids_tensor,
            "attention_mask" => mask_tensor,
        ])
        .map_err(|e| format!("Encoder inference: {e}"))?;

    let (enc_shape, enc_data) = enc_outputs["last_hidden_state"]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Encoder output: {e}"))?;
    let d_model = enc_shape[2] as usize;
    let enc_hidden: Vec<f32> = enc_data.to_vec();

    // Autoregressive decoding (no KV cache — simple, fast enough for short correction texts)
    let max_tokens = (input_token_count as f32 * 1.2) as usize + 16;
    let repeat_penalty = 1.5_f32;
    let mut generated: Vec<i64> = vec![ctx.decoder_start_id];

    for step in 0..max_tokens {
        if step >= 12 && is_looping(&generated, 6) {
            log::warn!("T5: loop detected at step {step}, stopping early");
            break;
        }

        let dec_seq_len = generated.len();

        let dec_ids = OrtTensor::from_array(([1usize, dec_seq_len], generated.clone()))
            .map_err(|e| format!("Decoder ids: {e}"))?;
        let enc_tensor =
            OrtTensor::from_array(([1usize, enc_seq_len, d_model], enc_hidden.clone()))
                .map_err(|e| format!("Enc hidden: {e}"))?;
        let enc_mask = OrtTensor::from_array(([1usize, enc_seq_len], attention_mask.clone()))
            .map_err(|e| format!("Enc mask: {e}"))?;

        let dec_outputs = ctx
            .decoder
            .run(ort::inputs![
                "input_ids" => dec_ids,
                "encoder_hidden_states" => enc_tensor,
                "encoder_attention_mask" => enc_mask,
            ])
            .map_err(|e| format!("Decoder step {step}: {e}"))?;

        let (logits_shape, logits_data) = dec_outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Logits: {e}"))?;
        let vocab_size = logits_shape[2] as usize;
        let last_offset = (dec_seq_len - 1) * vocab_size;
        let last_logits = &logits_data[last_offset..last_offset + vocab_size];

        // Apply repeat penalty + n-gram blocking
        let mut logits_vec: Vec<f32> = last_logits.to_vec();
        for &tid in &generated {
            let idx = tid as usize;
            if idx < logits_vec.len() {
                if logits_vec[idx] > 0.0 {
                    logits_vec[idx] /= repeat_penalty;
                } else {
                    logits_vec[idx] *= repeat_penalty;
                }
            }
        }
        for &tid in &calc_banned_ngram_tokens(&generated, 4) {
            if (tid as usize) < logits_vec.len() {
                logits_vec[tid as usize] = f32::NEG_INFINITY;
            }
        }

        // Greedy argmax
        let next_token = logits_vec
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i as i64)
            .unwrap_or(ctx.eos_id);

        if next_token == ctx.eos_id {
            break;
        }

        generated.push(next_token);
    }

    // Decode output tokens (skip decoder_start_id)
    let output_ids: Vec<u32> = if generated.len() > 1 {
        generated[1..].iter().map(|&id| id as u32).collect()
    } else {
        Vec::new()
    };

    let output_text = ctx
        .tokenizer
        .decode(&output_ids, true)
        .map_err(|e| format!("Token decoding failed: {e}"))?;

    let result = output_text.trim().to_string();
    if result.is_empty() {
        log::warn!("T5 correction returned empty output, keeping original");
        return Ok(text.to_string());
    }

    let result = strip_repetition(&result);

    // If output is significantly longer than input, likely echoed/duplicated
    if result.len() > input_len * 5 / 4 {
        let input_lower = text.trim().to_lowercase();
        let result_lower = result.to_lowercase();
        if result_lower.starts_with(&input_lower[..input_lower.len().min(20)]) {
            let input_words: Vec<&str> = text.split_whitespace().collect();
            let result_words: Vec<&str> = result.split_whitespace().collect();
            if result_words.len() > input_words.len() * 5 / 4 {
                log::warn!(
                    "T5: output echoed input ({} → {} words), keeping original",
                    input_words.len(),
                    result_words.len()
                );
                return Ok(text.to_string());
            }
        }
    }

    // Output shouldn't be much longer than input for correction tasks
    let max_len = std::cmp::max(input_len * 3 / 2, 100);
    if result.len() > max_len {
        log::warn!(
            "T5 correction output too long (len={} vs input={}, max={}), keeping original",
            result.len(),
            input_len,
            max_len
        );
        return Ok(text.to_string());
    }

    Ok(result)
}

/// Detect token-level looping: if the last `window` tokens repeat a pattern
/// that already appeared earlier, the model is stuck in a loop.
fn is_looping(generated: &[i64], window: usize) -> bool {
    if generated.len() < window * 2 {
        return false;
    }
    let tail = &generated[generated.len() - window..];
    let before = &generated[..generated.len() - window];
    before.windows(window).any(|w| w == tail)
}

/// Detect and strip repeated phrases/sentences from output text.
fn strip_repetition(text: &str) -> String {
    let text = text.trim();
    if text.len() < 20 {
        return text.to_string();
    }

    // Word-level half-split: check if the second half mirrors the first half
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() >= 6 {
        for pattern_len in (3..=words.len() / 2).rev() {
            let pattern = &words[..pattern_len];
            let rest = &words[pattern_len..];
            if rest.len() >= pattern_len {
                let compare = &rest[..pattern_len];
                let matching = pattern
                    .iter()
                    .zip(compare.iter())
                    .filter(|(a, b)| a.to_lowercase() == b.to_lowercase())
                    .count();
                if matching >= pattern_len * 75 / 100 {
                    log::warn!(
                        "T5: word-level repetition (pattern={}/{} words, {}% match), keeping first",
                        pattern_len,
                        words.len(),
                        matching * 100 / pattern_len
                    );
                    return words[..pattern_len].join(" ");
                }
            }
        }
    }

    // Sentence-level: split on sentence-ending punctuation
    let sentences: Vec<&str> = text
        .split(['.', '!', '?'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if sentences.len() >= 3 {
        let first = sentences[0].to_lowercase();
        let repeat_count = sentences
            .iter()
            .filter(|&&s| {
                let s_lower = s.to_lowercase();
                let len = first.len().min(s_lower.len());
                if len < 5 {
                    return false;
                }
                let matching = first
                    .chars()
                    .zip(s_lower.chars())
                    .filter(|(a, b)| a == b)
                    .count();
                matching >= len * 75 / 100
            })
            .count();

        if repeat_count >= 2 {
            log::warn!(
                "T5: sentence repetition ({}/{} match), keeping first",
                repeat_count,
                sentences.len()
            );
            let first_end = text.find(sentences[0]).unwrap_or(0) + sentences[0].len();
            let end = text[first_end..]
                .find(['.', '!', '?'])
                .map(|i| first_end + i + 1)
                .unwrap_or(first_end);
            return text[..end].trim().to_string();
        }
    }

    text.to_string()
}

fn calc_banned_ngram_tokens(generated: &[i64], ngram_size: usize) -> Vec<i64> {
    if generated.len() < ngram_size {
        return Vec::new();
    }
    let ngram_prefix = &generated[generated.len() - (ngram_size - 1)..];
    let mut banned = Vec::new();
    for i in 0..generated.len() - (ngram_size - 1) {
        if generated[i..i + ngram_size - 1] == *ngram_prefix {
            banned.push(generated[i + ngram_size - 1]);
        }
    }
    banned
}
