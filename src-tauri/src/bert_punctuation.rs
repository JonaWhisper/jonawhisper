use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

use crate::punct_common;

const TOKENIZER_FILENAME: &str = "tokenizer.json";
const TOKENIZER_URL: &str =
    "https://huggingface.co/ldenoue/fullstop-punctuation-multilang-large/resolve/main/tokenizer.json";

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
            punct_common::download_file(TOKENIZER_URL, &tokenizer_path)?;
        }

        let n_threads = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);

        let session = crate::ort_session::build_session(n_threads)?
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
    punct_common::restore_punctuation_windowed(text, |words| infer_chunk(ctx, words))
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
