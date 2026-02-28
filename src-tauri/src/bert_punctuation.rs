use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

const TOKENIZER_FILENAME: &str = "tokenizer.json";
const TOKENIZER_URL: &str =
    "https://huggingface.co/ldenoue/fullstop-punctuation-multilang-large/resolve/main/tokenizer.json";

/// Punctuation labels predicted by the fullstop-punctuation model.
/// Index 0 = no punctuation, 1..5 = punctuation characters.
const PUNCT_LABELS: &[&str] = &["", ".", ",", "?", "-", ":"];

const WINDOW_SIZE: usize = 230;
const OVERLAP: usize = 5;

/// Cached BERT context: ONNX session + tokenizer, reused across calls.
pub struct BertContext {
    session: Session,
    tokenizer: Tokenizer,
    model_id: String,
}

impl BertContext {
    /// Load an ONNX punctuation model and its tokenizer from disk.
    /// The tokenizer is auto-downloaded if not present alongside the model.
    pub fn load(model_path: &Path, model_id: &str) -> Result<Self, String> {
        let model_dir = model_path
            .parent()
            .ok_or_else(|| "Invalid model path".to_string())?;

        // Ensure tokenizer exists alongside the model
        let tokenizer_path = model_dir.join(TOKENIZER_FILENAME);
        if !tokenizer_path.exists() {
            download_tokenizer(&tokenizer_path)?;
        }

        let n_threads = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);

        let session = Session::builder()
            .map_err(|e| format!("Failed to create ONNX session builder: {e}"))?
            .with_intra_threads(n_threads)
            .map_err(|e| format!("Failed to set thread count: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load ONNX model: {e}"))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer: {e}"))?;

        log::info!(
            "BERT punctuation model loaded: {} ({})",
            model_id,
            model_path.display()
        );

        Ok(Self {
            session,
            tokenizer,
            model_id: model_id.to_string(),
        })
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }
}

/// Restore punctuation in a text using the BERT model.
///
/// Input: text with minimal/no punctuation (e.g. from Whisper).
/// Output: text with punctuation restored (periods, commas, question marks, etc.).
pub fn restore_punctuation(ctx: &mut BertContext, text: &str) -> Result<String, String> {
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

/// Run BERT inference on a chunk of words, return per-word label indices.
fn infer_chunk(ctx: &mut BertContext, words: &[String]) -> Result<Vec<usize>, String> {
    let chunk_text = words.join(" ");
    let n_words = words.len();

    let encoding = ctx
        .tokenizer
        .encode(chunk_text.as_str(), false)
        .map_err(|e| format!("Tokenization failed: {e}"))?;

    let ids = encoding.get_ids();
    let word_ids = encoding.get_word_ids();
    let seq_len = ids.len();

    let input_ids: Vec<i64> = ids.iter().map(|&id| id as i64).collect();
    let attention_mask: Vec<i64> = vec![1i64; seq_len];

    let ids_tensor = Tensor::from_array(([1usize, seq_len], input_ids))
        .map_err(|e| format!("Tensor creation failed: {e}"))?;
    let mask_tensor = Tensor::from_array(([1usize, seq_len], attention_mask))
        .map_err(|e| format!("Tensor creation failed: {e}"))?;

    let outputs = ctx
        .session
        .run(ort::inputs![
            "input_ids" => ids_tensor,
            "attention_mask" => mask_tensor,
        ])
        .map_err(|e| format!("ONNX inference failed: {e}"))?;

    let (shape, logits_data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("Failed to extract logits: {e}"))?;

    // logits shape: [1, seq_len, num_labels]
    let num_labels = *shape.last().unwrap_or(&6) as usize;

    // Map subword predictions back to words.
    // For each word, take the label from the LAST subword token (matches oliverguhr training).
    let mut labels = vec![0usize; n_words];
    for (token_idx, word_id) in word_ids.iter().enumerate() {
        if let Some(wid) = word_id {
            let wid = *wid as usize;
            if wid < n_words {
                let offset = token_idx * num_labels;
                let mut best_label = 0;
                let mut best_score = f32::NEG_INFINITY;
                for l in 0..num_labels {
                    let score = logits_data[offset + l];
                    if score > best_score {
                        best_score = score;
                        best_label = l;
                    }
                }
                // Last subword token wins (overwrites earlier subwords of same word)
                labels[wid] = best_label;
            }
        }
    }

    Ok(labels)
}

/// Download the HuggingFace tokenizer.json file.
fn download_tokenizer(path: &Path) -> Result<(), String> {
    log::info!("Downloading BERT tokenizer to {}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    let response = reqwest::blocking::get(TOKENIZER_URL)
        .map_err(|e| format!("Failed to download tokenizer: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Tokenizer download failed with status {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read tokenizer response: {e}"))?;
    std::fs::write(path, &bytes).map_err(|e| format!("Failed to write tokenizer: {e}"))?;
    log::info!("Tokenizer downloaded ({} bytes)", bytes.len());
    Ok(())
}
