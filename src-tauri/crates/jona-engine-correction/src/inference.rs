use std::path::Path;

use candle_core::{DType, Device, Tensor, D};
use candle_nn::VarBuilder;
use candle_transformers::models::t5::{self, T5ForConditionalGeneration};
use tokenizers::Tokenizer;

/// Cached T5 context: model + tokenizer + device, reused across calls.
pub struct T5Context {
    model: T5ForConditionalGeneration,
    tokenizer: Tokenizer,
    device: Device,
    config: t5::Config,
    model_id: String,
}

impl T5Context {
    /// Load a T5 correction model from a directory containing model.safetensors, config.json, tokenizer.json.
    pub fn load(model_dir: &Path, model_id: &str) -> Result<Self, String> {
        let config_path = model_dir.join("config.json");
        let tokenizer_path = model_dir.join("tokenizer.json");
        let model_path = model_dir.join("model.safetensors");

        let config_data = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.json: {e}"))?;
        let mut config: t5::Config = serde_json::from_str(&config_data)
            .map_err(|e| format!("Failed to parse config.json: {e}"))?;
        config.use_cache = true;

        let device = Device::new_metal(0).unwrap_or_else(|e| {
            log::info!("Metal unavailable ({e}), falling back to CPU");
            Device::Cpu
        });
        log::info!(
            "T5 correction device: {}",
            if matches!(device, Device::Cpu) { "CPU" } else { "Metal GPU" }
        );

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[&model_path], DType::F32, &device)
                .map_err(|e| format!("Failed to load safetensors: {e}"))?
        };

        let model = T5ForConditionalGeneration::load(vb, &config)
            .map_err(|e| format!("Failed to build T5 model: {e}"))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer: {e}"))?;

        log::info!(
            "T5 correction model loaded: {} ({})",
            model_id,
            model_dir.display()
        );

        Ok(Self {
            model,
            tokenizer,
            device,
            config,
            model_id: model_id.to_string(),
        })
    }
}

impl jona_types::HasModelId for T5Context {
    fn model_id(&self) -> &str {
        &self.model_id
    }
}

/// Detect and strip repetition.
fn strip_repetition(text: &str) -> String {
    let text = text.trim();
    let len = text.len();
    if len < 20 {
        return text.to_string();
    }

    let bytes = text.as_bytes();
    let mid = len / 2;
    let range_start = mid.saturating_sub(len / 10);
    let range_end = (mid + len / 10).min(len.saturating_sub(1));

    for i in range_start..=range_end {
        if bytes[i] != b' ' {
            continue;
        }
        let first = &text[..i];
        let rest = text[i + 1..].trim_start();

        let first_chars: Vec<char> = first.chars().take(15).collect();
        let rest_chars: Vec<char> = rest.chars().take(15).collect();
        let compare_len = first_chars.len().min(rest_chars.len());

        if compare_len >= 8 {
            let matching = first_chars[..compare_len]
                .iter()
                .zip(rest_chars[..compare_len].iter())
                .filter(|(a, b)| a == b)
                .count();
            if matching >= compare_len * 80 / 100 {
                log::warn!(
                    "T5 correction: repetition detected, keeping first half ({} -> {} chars)",
                    len,
                    first.trim_end().len()
                );
                return first.trim_end().to_string();
            }
        }
    }

    text.to_string()
}

fn calc_banned_ngram_tokens(generated: &[u32], ngram_size: usize) -> Vec<u32> {
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

/// Run T5 correction on a text. Returns the corrected text.
pub fn correct(ctx: &mut T5Context, text: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(text.to_string());
    }

    let input_len = text.len();

    ctx.model.clear_kv_cache();

    let encoding = ctx
        .tokenizer
        .encode(text, false)
        .map_err(|e| format!("Tokenization failed: {e}"))?;
    let input_ids: Vec<u32> = encoding.get_ids().to_vec();
    let input_token_count = input_ids.len();

    let input_tensor = Tensor::new(input_ids.as_slice(), &ctx.device)
        .and_then(|t| t.unsqueeze(0))
        .map_err(|e| format!("Input tensor creation failed: {e}"))?;

    let encoder_output = ctx
        .model
        .encode(&input_tensor)
        .map_err(|e| format!("T5 encoding failed: {e}"))?;

    let eos_token_id = ctx.config.eos_token_id as u32;
    let decoder_start_id = ctx
        .config
        .decoder_start_token_id
        .unwrap_or(ctx.config.pad_token_id) as u32;
    let max_tokens = (input_token_count as f32 * 1.5) as usize + 32;
    let repeat_penalty = 1.2_f64;

    let mut generated_ids: Vec<u32> = vec![decoder_start_id];

    for _ in 0..max_tokens {
        let decoder_input = Tensor::new(&generated_ids[generated_ids.len() - 1..], &ctx.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| format!("Decoder input tensor failed: {e}"))?;

        let logits = ctx
            .model
            .decode(&decoder_input, &encoder_output)
            .map_err(|e| format!("T5 decoding failed: {e}"))?;

        let logits = logits
            .to_device(&Device::Cpu)
            .map_err(|e| format!("Failed to move logits to CPU: {e}"))?
            .squeeze(0)
            .map_err(|e| format!("Failed to squeeze logits: {e}"))?;

        let logits = if repeat_penalty != 1.0 {
            let logits_vec: Vec<f32> = logits
                .to_vec1()
                .map_err(|e| format!("Failed to extract logits: {e}"))?;
            let mut modified = logits_vec;
            for &token_id in &generated_ids {
                let idx = token_id as usize;
                if idx < modified.len() {
                    if modified[idx] > 0.0 {
                        modified[idx] /= repeat_penalty as f32;
                    } else {
                        modified[idx] *= repeat_penalty as f32;
                    }
                }
            }
            Tensor::new(modified.as_slice(), &Device::Cpu)
                .map_err(|e| format!("Failed to create modified logits: {e}"))?
        } else {
            logits
        };

        let logits = {
            let banned = calc_banned_ngram_tokens(&generated_ids, 3);
            if banned.is_empty() {
                logits
            } else {
                let mut v: Vec<f32> = logits
                    .to_vec1()
                    .map_err(|e| format!("Failed to extract logits for ngram blocking: {e}"))?;
                for &token_id in &banned {
                    if (token_id as usize) < v.len() {
                        v[token_id as usize] = f32::NEG_INFINITY;
                    }
                }
                Tensor::new(v.as_slice(), &Device::Cpu)
                    .map_err(|e| format!("Failed to create ngram-blocked logits: {e}"))?
            }
        };

        let next_token = logits
            .argmax(D::Minus1)
            .map_err(|e| format!("Argmax failed: {e}"))?
            .to_scalar::<u32>()
            .map_err(|e| format!("Failed to extract token: {e}"))?;

        if next_token == eos_token_id {
            break;
        }

        generated_ids.push(next_token);
    }

    let output_ids: Vec<u32> = if generated_ids.len() > 1 {
        generated_ids[1..].to_vec()
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

    let max_len = std::cmp::max(input_len * 2, 200);
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
