use std::path::Path;

use candle_core::{DType, Device, Module, Tensor};
use candle_nn::{linear, Linear, VarBuilder};
use candle_transformers::models::xlm_roberta::{Config, XLMRobertaModel};
use tokenizers::Tokenizer;

const TOKENIZER_FILENAME: &str = "tokenizer.json";
const CONFIG_FILENAME: &str = "config.json";
const TOKENIZER_URL: &str =
    "https://huggingface.co/oliverguhr/fullstop-punctuation-multilingual-base/resolve/main/tokenizer.json";
const CONFIG_URL: &str =
    "https://huggingface.co/oliverguhr/fullstop-punctuation-multilingual-base/resolve/main/config.json";

/// Punctuation labels predicted by the fullstop-punctuation model.
/// Index 0 = no punctuation, 1..5 = punctuation characters.
const PUNCT_LABELS: &[&str] = &["", ".", ",", "?", "-", ":"];
const NUM_LABELS: usize = 6;

const WINDOW_SIZE: usize = 230;
const OVERLAP: usize = 5;

/// XLM-RoBERTa with a token classification head (linear on top of hidden states).
/// Not provided by candle-transformers, so we build it from the base model + Linear.
struct XLMRobertaForTokenClassification {
    roberta: XLMRobertaModel,
    classifier: Linear,
}

impl XLMRobertaForTokenClassification {
    fn new(num_labels: usize, cfg: &Config, vb: VarBuilder) -> candle_core::Result<Self> {
        let roberta = XLMRobertaModel::new(cfg, vb.pp("roberta"))?;
        let classifier = linear(cfg.hidden_size, num_labels, vb.pp("classifier"))?;
        Ok(Self {
            roberta,
            classifier,
        })
    }

    fn forward(
        &self,
        input_ids: &Tensor,
        attention_mask: &Tensor,
        token_type_ids: &Tensor,
    ) -> candle_core::Result<Tensor> {
        let hidden = self.roberta.forward(
            input_ids,
            attention_mask,
            token_type_ids,
            None,
            None,
            None,
        )?;
        self.classifier.forward(&hidden)
    }
}

/// Cached candle context: model + tokenizer + device, reused across calls.
pub struct CandlePunctContext {
    model: XLMRobertaForTokenClassification,
    tokenizer: Tokenizer,
    device: Device,
    model_id: String,
}

impl CandlePunctContext {
    /// Load a safetensors punctuation model and its tokenizer from disk.
    /// Tokenizer and config are auto-downloaded if not present alongside the model.
    pub fn load(model_path: &Path, model_id: &str) -> Result<Self, String> {
        let model_dir = model_path
            .parent()
            .ok_or_else(|| "Invalid model path".to_string())?;

        // Ensure tokenizer.json exists alongside the model
        let tokenizer_path = model_dir.join(TOKENIZER_FILENAME);
        if !tokenizer_path.exists() {
            download_file(TOKENIZER_URL, &tokenizer_path)?;
        }

        // Ensure config.json exists alongside the model
        let config_path = model_dir.join(CONFIG_FILENAME);
        if !config_path.exists() {
            download_file(CONFIG_URL, &config_path)?;
        }

        // Load config
        let config_data = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.json: {e}"))?;
        let config: Config = serde_json::from_str(&config_data)
            .map_err(|e| format!("Failed to parse config.json: {e}"))?;

        // Select device: Metal GPU with CPU fallback
        let device = Device::new_metal(0).unwrap_or_else(|e| {
            log::info!("Metal unavailable ({e}), falling back to CPU");
            Device::Cpu
        });
        log::info!(
            "Candle device: {}",
            if matches!(device, Device::Cpu) {
                "CPU"
            } else {
                "Metal GPU"
            }
        );

        // Load safetensors weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[model_path], DType::F32, &device)
                .map_err(|e| format!("Failed to load safetensors: {e}"))?
        };

        // Build model
        let model = XLMRobertaForTokenClassification::new(NUM_LABELS, &config, vb)
            .map_err(|e| format!("Failed to build model: {e}"))?;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer: {e}"))?;

        log::info!(
            "Candle punctuation model loaded: {} ({})",
            model_id,
            model_path.display()
        );

        Ok(Self {
            model,
            tokenizer,
            device,
            model_id: model_id.to_string(),
        })
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }
}

/// Restore punctuation in a text using the candle BERT model.
///
/// Input: text with minimal/no punctuation (e.g. from Whisper).
/// Output: text with punctuation restored (periods, commas, question marks, etc.).
pub fn restore_punctuation(ctx: &CandlePunctContext, text: &str) -> Result<String, String> {
    let words = strip_and_split(text);
    if words.is_empty() {
        return Ok(String::new());
    }

    let mut labels: Vec<usize> = vec![0; words.len()];
    let mut offset = 0;

    while offset < words.len() {
        let end = (offset + WINDOW_SIZE).min(words.len());
        let chunk = &words[offset..end];

        let chunk_labels = infer_chunk(ctx, chunk)?;

        // Merge: skip overlap words for non-first windows
        let start_word = if offset == 0 { 0 } else { OVERLAP };
        for (i, &label) in chunk_labels.iter().enumerate() {
            if i >= start_word {
                let global_idx = offset + i;
                if global_idx < words.len() {
                    labels[global_idx] = label;
                }
            }
        }

        if end >= words.len() {
            break;
        }
        offset += WINDOW_SIZE - OVERLAP;
    }

    // Reconstruct text with punctuation
    let mut result = String::new();
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        result.push_str(word);
        let label = labels[i];
        if label > 0 && label < PUNCT_LABELS.len() {
            result.push_str(PUNCT_LABELS[label]);
        }
    }

    Ok(result)
}

/// Strip existing punctuation and split into words.
fn strip_and_split(text: &str) -> Vec<String> {
    text.chars()
        .filter(|c| !matches!(c, '.' | ',' | '?' | ':' | '-' | ';' | '!'))
        .collect::<String>()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Run candle inference on a chunk of words, return per-word label indices.
fn infer_chunk(ctx: &CandlePunctContext, words: &[String]) -> Result<Vec<usize>, String> {
    let chunk_text = words.join(" ");
    let n_words = words.len();

    let encoding = ctx
        .tokenizer
        .encode(chunk_text.as_str(), false)
        .map_err(|e| format!("Tokenization failed: {e}"))?;

    let ids = encoding.get_ids();
    let word_ids = encoding.get_word_ids();
    let seq_len = ids.len();

    let input_ids: Vec<u32> = ids.to_vec();
    let attention_mask: Vec<u32> = vec![1u32; seq_len];
    let token_type_ids: Vec<u32> = vec![0u32; seq_len];

    let ids_tensor = Tensor::new(&input_ids[..], &ctx.device)
        .and_then(|t| t.unsqueeze(0))
        .map_err(|e| format!("Tensor creation failed: {e}"))?;
    let mask_tensor = Tensor::new(&attention_mask[..], &ctx.device)
        .and_then(|t| t.unsqueeze(0))
        .map_err(|e| format!("Tensor creation failed: {e}"))?;
    let type_tensor = Tensor::new(&token_type_ids[..], &ctx.device)
        .and_then(|t| t.unsqueeze(0))
        .map_err(|e| format!("Tensor creation failed: {e}"))?;

    let logits = ctx
        .model
        .forward(&ids_tensor, &mask_tensor, &type_tensor)
        .map_err(|e| format!("Candle inference failed: {e}"))?;

    // logits shape: [1, seq_len, num_labels] — move to CPU for extraction
    let logits = logits
        .to_device(&Device::Cpu)
        .map_err(|e| format!("Failed to move logits to CPU: {e}"))?;
    let logits = logits
        .squeeze(0)
        .map_err(|e| format!("Failed to squeeze logits: {e}"))?;
    // logits shape: [seq_len, num_labels]

    let argmax = logits
        .argmax(1)
        .map_err(|e| format!("Argmax failed: {e}"))?;
    let predictions: Vec<u32> = argmax
        .to_vec1()
        .map_err(|e| format!("Failed to extract predictions: {e}"))?;

    // Map subword predictions back to words.
    // For each word, take the label from the LAST subword token (matches oliverguhr training).
    let mut labels = vec![0usize; n_words];
    for (token_idx, word_id) in word_ids.iter().enumerate() {
        if let Some(wid) = word_id {
            let wid = *wid as usize;
            if wid < n_words && token_idx < predictions.len() {
                // Last subword token wins (overwrites earlier subwords of same word)
                labels[wid] = predictions[token_idx] as usize;
            }
        }
    }

    Ok(labels)
}

/// Download a file from a URL.
fn download_file(url: &str, path: &Path) -> Result<(), String> {
    log::info!("Downloading {} to {}", url, path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    let response = reqwest::blocking::get(url)
        .map_err(|e| format!("Failed to download {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Download failed with status {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response: {e}"))?;
    std::fs::write(path, &bytes).map_err(|e| format!("Failed to write file: {e}"))?;
    log::info!("Downloaded {} ({} bytes)", path.display(), bytes.len());
    Ok(())
}
