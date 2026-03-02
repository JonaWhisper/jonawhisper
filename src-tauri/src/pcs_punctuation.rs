use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

use crate::punct_common;

// -- SentencePiece protobuf parsing (prost) --

/// Minimal SentencePiece ModelProto — only the fields we need.
#[derive(prost::Message)]
struct ModelProto {
    #[prost(message, repeated, tag = "1")]
    pieces: Vec<SentencePieceProto>,
}

#[derive(prost::Message)]
struct SentencePieceProto {
    #[prost(string, optional, tag = "1")]
    piece: Option<String>,
    #[prost(float, optional, tag = "2")]
    score: Option<f32>,
}

// -- Constants --

const SPE_MODEL_FILENAME: &str = "spe_unigram_64k_lowercase_47lang.model";
const SPE_MODEL_URL: &str =
    "https://huggingface.co/1-800-BAD-CODE/punct_cap_seg_47_language/resolve/main/spe_unigram_64k_lowercase_47lang.model";
const TOKENIZER_CACHE_FILENAME: &str = "tokenizer.json";

const MAX_SEQ_LEN: usize = 128;
const OVERLAP_TOKENS: usize = 16;

/// Post-token punctuation labels (16 classes).
const POST_LABELS: &[&str] = &[
    "", ".", ",", "?", "？", "，", "。", "、", "・", "।", "؟", "،", ";", "።", "፣", "፧",
];

/// Pre-token punctuation labels (2 classes).
const PRE_LABELS: &[&str] = &["", "¿"];

// -- PCS Context --

/// Cached PCS context: ONNX session + tokenizer, reused across calls.
pub struct PcsContext {
    session: Session,
    tokenizer: Tokenizer,
    bos_id: i64,
    eos_id: i64,
    model_id: String,
}

impl PcsContext {
    /// Load the PCS ONNX model and its tokenizer from disk.
    /// The SentencePiece .model is auto-downloaded and converted to tokenizer.json on first use.
    pub fn load(model_path: &Path, model_id: &str) -> Result<Self, String> {
        let model_dir = model_path
            .parent()
            .ok_or_else(|| "Invalid model path".to_string())?;

        // Ensure tokenizer exists (convert from SentencePiece .model if needed)
        let tokenizer_path = model_dir.join(TOKENIZER_CACHE_FILENAME);
        let tokenizer = if tokenizer_path.exists() {
            Tokenizer::from_file(&tokenizer_path)
                .map_err(|e| format!("Failed to load cached tokenizer: {e}"))?
        } else {
            let tok = build_tokenizer_from_spe(model_dir)?;
            // Cache for future loads
            tok.save(&tokenizer_path, true)
                .map_err(|e| format!("Failed to cache tokenizer: {e}"))?;
            log::info!("PCS tokenizer cached to {}", tokenizer_path.display());
            tok
        };

        // Extract BOS/EOS IDs from vocabulary
        let vocab = tokenizer.get_vocab(false);
        let bos_id = *vocab.get("<s>").ok_or("BOS token <s> not found in vocab")? as i64;
        let eos_id = *vocab.get("</s>").ok_or("EOS token </s> not found in vocab")? as i64;
        log::info!("PCS tokenizer: BOS={}, EOS={}, vocab_size={}", bos_id, eos_id, vocab.len());

        let n_threads = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);

        let session = crate::ort_session::build_session(n_threads)?
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load PCS ONNX model: {e}"))?;

        log::info!(
            "PCS punctuation model loaded: {} ({})",
            model_id,
            model_path.display()
        );

        Ok(Self {
            session,
            tokenizer,
            bos_id,
            eos_id,
            model_id: model_id.to_string(),
        })
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }
}

/// Restore punctuation, capitalization, and segmentation using the PCS model.
///
/// Input: text with minimal/no punctuation (e.g. from Whisper).
/// Output: text with punctuation, proper capitalization, and sentence segmentation.
pub fn restore_punctuation_and_case(ctx: &mut PcsContext, text: &str) -> Result<String, String> {
    let input_text = text.trim();
    if input_text.is_empty() {
        return Ok(String::new());
    }

    // Strip existing punctuation and lowercase (model expects lowercase input)
    let cleaned = strip_punctuation(input_text).to_lowercase();
    if cleaned.trim().is_empty() {
        return Ok(String::new());
    }

    // Tokenize
    let encoding = ctx
        .tokenizer
        .encode(cleaned.as_str(), false)
        .map_err(|e| format!("PCS tokenization failed: {e}"))?;

    let all_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
    let all_tokens: Vec<String> = encoding
        .get_tokens()
        .iter()
        .map(|t| t.to_string())
        .collect();

    if all_ids.is_empty() {
        return Ok(String::new());
    }

    // Sliding window inference (max 126 content tokens per window, +2 for BOS/EOS)
    let content_window = MAX_SEQ_LEN - 2;
    let mut all_pre_preds: Vec<usize> = vec![0; all_ids.len()];
    let mut all_post_preds: Vec<usize> = vec![0; all_ids.len()];
    let mut all_cap_preds: Vec<Vec<bool>> = vec![vec![false; 16]; all_ids.len()];

    let mut offset = 0;
    while offset < all_ids.len() {
        let end = (offset + content_window).min(all_ids.len());
        let chunk_ids = &all_ids[offset..end];

        let (pre_preds, post_preds, cap_preds) = infer_window(ctx, chunk_ids)?;

        // Merge predictions: for overlapping regions, prefer the later window's
        // predictions for the first half of overlap, earlier window's for second half.
        let merge_start = if offset == 0 { 0 } else { OVERLAP_TOKENS / 2 };
        for i in merge_start..pre_preds.len() {
            let global_idx = offset + i;
            if global_idx < all_ids.len() {
                all_pre_preds[global_idx] = pre_preds[i];
                all_post_preds[global_idx] = post_preds[i];
                all_cap_preds[global_idx] = cap_preds[i].clone();
            }
        }

        if end >= all_ids.len() {
            break;
        }
        offset += content_window - OVERLAP_TOKENS;
    }

    // Reconstruct text using token pieces + predictions
    let result = reconstruct_text(&all_tokens, &all_pre_preds, &all_post_preds, &all_cap_preds);
    Ok(result)
}

/// Run ONNX inference on a single window of token IDs.
/// Returns (pre_preds, post_preds, cap_preds) for the content tokens (excluding BOS/EOS).
fn infer_window(
    ctx: &mut PcsContext,
    content_ids: &[i64],
) -> Result<(Vec<usize>, Vec<usize>, Vec<Vec<bool>>), String> {
    let seq_len = content_ids.len() + 2; // +BOS +EOS

    // Build input: [BOS] + content + [EOS]
    let mut input_ids = Vec::with_capacity(seq_len);
    input_ids.push(ctx.bos_id);
    input_ids.extend_from_slice(content_ids);
    input_ids.push(ctx.eos_id);

    let ids_tensor = Tensor::from_array(([1usize, seq_len], input_ids))
        .map_err(|e| format!("Tensor creation failed: {e}"))?;

    let outputs = ctx
        .session
        .run(ort::inputs!["input_ids" => ids_tensor])
        .map_err(|e| format!("PCS ONNX inference failed: {e}"))?;

    // Output order: pre_preds[0], post_preds[1], cap_preds[2], seg_preds[3]
    let n_content = content_ids.len();

    // pre_preds: [1, seq_len] int64
    let (_, pre_data) = outputs[0]
        .try_extract_tensor::<i64>()
        .map_err(|e| format!("Failed to extract pre_preds: {e}"))?;
    let pre_preds: Vec<usize> = pre_data[1..=n_content]
        .iter()
        .map(|&v| v as usize)
        .collect();

    // post_preds: [1, seq_len] int64
    let (_, post_data) = outputs[1]
        .try_extract_tensor::<i64>()
        .map_err(|e| format!("Failed to extract post_preds: {e}"))?;
    let post_preds: Vec<usize> = post_data[1..=n_content]
        .iter()
        .map(|&v| v as usize)
        .collect();

    // cap_preds: [1, seq_len, 16] bool
    let (_, cap_data) = outputs[2]
        .try_extract_tensor::<bool>()
        .map_err(|e| format!("Failed to extract cap_preds: {e}"))?;
    let cap_preds: Vec<Vec<bool>> = (1..=n_content)
        .map(|i| {
            let start = i * 16;
            cap_data[start..start + 16].to_vec()
        })
        .collect();

    Ok((pre_preds, post_preds, cap_preds))
}

/// Reconstruct text from SentencePiece tokens with punctuation and capitalization predictions.
fn reconstruct_text(
    tokens: &[String],
    pre_preds: &[usize],
    post_preds: &[usize],
    cap_preds: &[Vec<bool>],
) -> String {
    let mut chars: Vec<char> = Vec::new();

    for (token_idx, token) in tokens.iter().enumerate() {
        let has_space_prefix = token.starts_with('▁');

        // Insert space before word-initial tokens (▁ prefix)
        if has_space_prefix && !chars.is_empty() {
            chars.push(' ');
        }

        // Character start index: skip the ▁ prefix if present
        let char_start = if has_space_prefix { '▁'.len_utf8() } else { 0 };
        let token_chars: Vec<char> = token[char_start..].chars().collect();
        let token_len = token_chars.len();

        for (char_idx, &ch) in token_chars.iter().enumerate() {
            // cap_preds index includes the ▁ prefix position
            let cap_idx = if has_space_prefix { char_idx + 1 } else { char_idx };

            // Pre-punctuation on first character of token
            if char_idx == 0 {
                let pre = pre_preds[token_idx];
                if pre < PRE_LABELS.len() && !PRE_LABELS[pre].is_empty() {
                    for c in PRE_LABELS[pre].chars() {
                        chars.push(c);
                    }
                }
            }

            // Apply capitalization
            if cap_idx < cap_preds[token_idx].len() && cap_preds[token_idx][cap_idx] {
                for uc in ch.to_uppercase() {
                    chars.push(uc);
                }
            } else {
                chars.push(ch);
            }

            // Post-punctuation on last character of token
            if char_idx == token_len - 1 {
                let post = post_preds[token_idx];
                if post < POST_LABELS.len() && !POST_LABELS[post].is_empty() {
                    for c in POST_LABELS[post].chars() {
                        chars.push(c);
                    }
                }
            }
        }
    }

    chars.into_iter().collect()
}

/// Strip existing punctuation characters from text.
fn strip_punctuation(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !matches!(
                c,
                '.' | ',' | '?' | ':' | '-' | ';' | '!' | '。' | '，' | '？'
                    | '、' | '・' | '।' | '؟' | '،' | '።' | '፣' | '፧' | '¿'
            )
        })
        .collect()
}

/// Build a `tokenizers::Tokenizer` by parsing the SentencePiece .model protobuf.
/// Downloads the .model if not present in the model directory.
fn build_tokenizer_from_spe(model_dir: &Path) -> Result<Tokenizer, String> {
    let spe_path = model_dir.join(SPE_MODEL_FILENAME);

    // Download .model if not present
    if !spe_path.exists() {
        log::info!("Downloading SentencePiece model to {}", spe_path.display());
        punct_common::download_file(SPE_MODEL_URL, &spe_path)?;
    }

    // Parse protobuf
    let data = std::fs::read(&spe_path)
        .map_err(|e| format!("Failed to read SentencePiece model: {e}"))?;
    let model: ModelProto = prost::Message::decode(data.as_slice())
        .map_err(|e| format!("Failed to parse SentencePiece protobuf: {e}"))?;

    log::info!("SentencePiece model: {} pieces", model.pieces.len());

    // Extract vocabulary: (piece, score) pairs
    let vocab: Vec<(String, f64)> = model
        .pieces
        .iter()
        .map(|p| {
            let piece = p.piece.clone().unwrap_or_default();
            let score = p.score.unwrap_or(0.0) as f64;
            (piece, score)
        })
        .collect();

    // Build Unigram tokenizer
    let unigram = tokenizers::models::unigram::Unigram::from(vocab, Some(0), false)
        .map_err(|e| format!("Failed to build Unigram model: {e}"))?;

    let mut tokenizer = Tokenizer::new(unigram);

    // Normalizer: NFC + Lowercase (model was trained on lowercased text)
    use tokenizers::NormalizerWrapper;
    use tokenizers::normalizers::{Lowercase, Sequence, unicode::NFC};
    tokenizer.with_normalizer(Some(Sequence::new(vec![
        NormalizerWrapper::NFC(NFC),
        NormalizerWrapper::Lowercase(Lowercase),
    ])));

    // Pre-tokenizer: Metaspace (SentencePiece ▁ convention)
    use tokenizers::decoders::metaspace::Metaspace;
    tokenizer.with_pre_tokenizer(Some(Metaspace::default()));

    log::info!("PCS tokenizer built from SentencePiece model");
    Ok(tokenizer)
}

