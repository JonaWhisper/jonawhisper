use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineError, EngineRegistration,
    GpuMode, Language, TranscriptionResult, tokens_to_word_confidences,
};
use ort::session::Session;
use ort::value::Tensor;
use std::any::Any;
use std::collections::HashMap;
use std::path::Path;

// -- Constants --

const MAX_DECODE_TOKENS: usize = 512;

/// The 25 European languages of Canary-1B-v2; the 180m flash model covers the first four.
const CANARY_V2_LANGS: &[(&str, &str)] = &[
    ("fr", "Fran\u{00e7}ais"), ("en", "English"), ("de", "Deutsch"), ("es", "Espa\u{00f1}ol"),
    ("it", "Italiano"), ("pt", "Portugu\u{00ea}s"), ("nl", "Nederlands"), ("pl", "Polski"),
    ("ru", "\u{0420}\u{0443}\u{0441}\u{0441}\u{043a}\u{0438}\u{0439}"),
    ("uk", "\u{0423}\u{043a}\u{0440}\u{0430}\u{0457}\u{043d}\u{0441}\u{044c}\u{043a}\u{0430}"),
    ("sv", "Svenska"), ("da", "Dansk"), ("fi", "Suomi"), ("ro", "Rom\u{00e2}n\u{0103}"),
    ("hu", "Magyar"), ("cs", "\u{010c}e\u{0161}tina"), ("sk", "Sloven\u{010d}ina"),
    ("bg", "\u{0411}\u{044a}\u{043b}\u{0433}\u{0430}\u{0440}\u{0441}\u{043a}\u{0438}"),
    ("hr", "Hrvatski"), ("sl", "Sloven\u{0161}\u{010d}ina"),
    ("el", "\u{0395}\u{03bb}\u{03bb}\u{03b7}\u{03bd}\u{03b9}\u{03ba}\u{03ac}"),
    ("lt", "Lietuvi\u{0173}"), ("lv", "Latvie\u{0161}u"), ("et", "Eesti"), ("mt", "Malti"),
];

// -- Context (cached model state) --

/// Cached Canary inference context: encoder + decoder ONNX sessions + vocabulary.
pub struct CanaryContext {
    encoder: Session,
    decoder: Session,
    vocab: Vec<String>,
    token_to_id: HashMap<String, i64>,
    is_sentencepiece: bool,
    /// `decoder_mems` layout, read from the decoder graph: 6 layers on
    /// canary-180m-flash, 10 on canary-1b-v2.
    mems_layers: usize,
    mems_hidden: usize,
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
}

// -- Loading --

/// Load Canary encoder + decoder sessions and vocabulary from a model directory.
pub fn load(model_dir: &Path) -> Result<CanaryContext, EngineError> {
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
    let decoder = jona_engines::ort_session::build_cpu_session(n_threads)
        .map_err(EngineError::LaunchFailed)?
        .commit_from_file(&decoder_path)
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to load decoder: {e}")))?;

    let vocab_text = std::fs::read_to_string(&vocab_path)
        .map_err(|e| EngineError::LaunchFailed(format!("Failed to read vocab: {e}")))?;
    let (vocab, token_to_id) = parse_vocab(&vocab_text)
        .map_err(EngineError::LaunchFailed)?;

    let (mems_layers, mems_hidden) = decoder_mems_dims(&decoder)
        .ok_or_else(|| EngineError::LaunchFailed("Decoder has no static decoder_mems shape".into()))?;

    let is_sentencepiece = vocab.iter().any(|t| t.contains('\u{2581}'));

    log::info!(
        "Canary loaded: {} vocab tokens, sentencepiece={}, decoder_mems={}x{}",
        vocab.len(), is_sentencepiece, mems_layers, mems_hidden
    );

    Ok(CanaryContext {
        encoder,
        decoder,
        vocab,
        token_to_id,
        is_sentencepiece,
        mems_layers,
        mems_hidden,
    })
}

fn decoder_mems_dims(decoder: &Session) -> Option<(usize, usize)> {
    let outlet = decoder.inputs().iter().find(|i| i.name() == "decoder_mems")?;
    let ort::value::ValueType::Tensor { shape, .. } = outlet.dtype() else {
        return None;
    };
    let layers = *shape.get(0)?;
    let hidden = *shape.get(3)?;
    (layers > 0 && hidden > 0).then_some((layers as usize, hidden as usize))
}

// -- Inference --

/// Transcribe an audio file using a loaded CanaryContext.
pub fn transcribe(ctx: &mut CanaryContext, audio_path: &Path, language: &str) -> Result<TranscriptionResult, EngineError> {
    let audio = jona_engines::audio::read_wav_f32(audio_path)?;

    // Compute mel spectrogram (Canary config: HTK mel scale)
    let (features, n_frames) = jona_engines::mel::extract_features(&audio);

    // Run encoder
    let enc_result = run_encoder(ctx, &features, n_frames)?;

    // Resolve language for prompt
    let lang = if language == "auto" { "en" } else { language };

    let prompt_tokens = build_prompt(ctx, lang);

    let output_tokens = run_decoder(ctx, &prompt_tokens, &enc_result)?;
    let text = decode_tokens_with_probs(ctx, &output_tokens);

    Ok(TranscriptionResult {
        text: text.0.trim().to_string(),
        word_confidences: tokens_to_word_confidences(&text.1),
    })
}

/// NeMo's canonical AED prompt. A short prompt still decodes on canary-180m-flash
/// but makes canary-1b-v2 drop its first word.
fn build_prompt(ctx: &CanaryContext, lang: &str) -> Vec<i64> {
    let mut tokens: Vec<i64> = Vec::with_capacity(10);
    let mut push = |token: &str| {
        if let Some(id) = ctx.token_id(token) {
            tokens.push(id);
        }
    };
    push("\u{2581}");
    push("<|startofcontext|>");
    push("<|startoftranscript|>");
    push("<|emo:undefined|>");
    let lang_token = format!("<|{lang}|>");
    push(&lang_token);
    push(&lang_token);
    push("<|pnc|>");
    push("<|noitn|>");
    push("<|notimestamp|>");
    push("<|nodiarize|>");

    if tokens.is_empty() {
        tokens.push(ctx.bos_id());
    }
    tokens
}

// -- Encoder --

struct EncoderResult {
    embeddings: Vec<f32>,
    emb_shape: [usize; 3],
    mask: Vec<i64>,
    mask_len: usize,
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
    })
}

// -- Decoder --

/// Token ID with its softmax probability.
type TokenWithProb = (i64, f32);

fn softmax_prob(logits: &[f32], token_idx: usize) -> f32 {
    let max_val = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let sum: f32 = logits.iter().map(|&x| (x - max_val).exp()).sum();
    ((logits[token_idx] - max_val).exp()) / sum
}

fn run_decoder(
    ctx: &mut CanaryContext,
    prompt_tokens: &[i64],
    enc: &EncoderResult,
) -> Result<Vec<TokenWithProb>, EngineError> {
    let eos_id = ctx.eos_id();
    let mut output_tokens: Vec<TokenWithProb> = Vec::new();

    let mut cache_data: Vec<f32> = Vec::new();
    let mut cache_shape: [usize; 4] = [ctx.mems_layers, 1, 0, ctx.mems_hidden];

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

        let prob = softmax_prob(last_logits, next_token as usize);
        output_tokens.push((next_token, prob));

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

/// Decode tokens into text + (token_text, probability) pairs for confidence scoring.
fn decode_tokens_with_probs(ctx: &CanaryContext, tokens: &[TokenWithProb]) -> (String, Vec<(String, f32)>) {
    let mut text = String::new();
    let mut token_probs: Vec<(String, f32)> = Vec::new();

    for &(id, prob) in tokens {
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
            token_probs.push((replaced.clone(), prob));
            text.push_str(&replaced);
        } else if let Some(stripped) = token.strip_prefix("##") {
            token_probs.push((stripped.to_string(), prob));
            text.push_str(stripped);
        } else if !text.is_empty() {
            token_probs.push((format!(" {token}"), prob));
            text.push(' ');
            text.push_str(token);
        } else {
            token_probs.push((token.clone(), prob));
            text.push_str(token);
        }
    }

    (text, token_probs)
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

impl ASREngine for CanaryEngine {
    fn engine_id(&self) -> &str { "canary" }
    fn display_name(&self) -> &str { "Canary" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "canary:1b-v2-int8".into(),
                engine_id: "canary".into(),
                label: "Canary 1B V2".into(),
                quantization: Some("INT8".into()),
                filename: "1b-v2-int8".into(),
                url: String::new(),
                size: 859_078_138 + 170_040_374 + 208_022,
                storage_dir: jona_types::engine_storage_dir("canary"),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/canary-1b-v2-onnx/resolve/main/encoder-model.int8.onnx".into(),
                            size: 859_078_138,
                        },
                        DownloadFile {
                            filename: "decoder-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/canary-1b-v2-onnx/resolve/main/decoder-model.int8.onnx".into(),
                            size: 170_040_374,
                        },
                        DownloadFile {
                            filename: "vocab.txt".into(),
                            url: "https://huggingface.co/istupakov/canary-1b-v2-onnx/resolve/main/vocab.txt".into(),
                            size: 208_022,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                wer: Some(2.18),
                rtf: Some(0.22),
                recommended_for: None,
                params: Some(0.978),
                ram: Some(1_450_000_000),
                lang_codes: Some(CANARY_V2_LANGS.iter().map(|(c, _)| (*c).into()).collect()),
                runtime: Some("ort".into()),
            },
            ASRModel {
                id: "canary:180m-flash-int8".into(),
                engine_id: "canary".into(),
                label: "Canary Flash".into(),
                quantization: Some("INT8".into()),
                filename: "180m-flash-int8".into(),
                url: String::new(),
                size: 213_284_662,
                storage_dir: jona_types::engine_storage_dir("canary"),
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
                recommended_for: None,
                params: Some(0.182),
                ram: Some(300_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "de".into(), "es".into()]),
                runtime: Some("ort".into()),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        CANARY_V2_LANGS
            .iter()
            .map(|(code, label)| Language { code: (*code).into(), label: (*label).into() })
            .collect()
    }

    fn description(&self) -> &str {
        "NVIDIA Canary encoder-decoder ASR. 180m flash for FR/EN/DE/ES, 1B v2 for 25 European languages."
    }

    fn create_context(&self, model: &ASRModel, _gpu_mode: GpuMode)
        -> Result<Box<dyn Any + Send>, EngineError>
    {
        let ctx = load(&model.local_path())?;
        Ok(Box::new(ctx))
    }

    fn transcribe(&self, ctx: &mut dyn Any, audio_path: &Path, language: &str)
        -> Result<TranscriptionResult, EngineError>
    {
        let ctx = ctx.downcast_mut::<CanaryContext>()
            .ok_or_else(|| EngineError::LaunchFailed("Invalid canary context".into()))?;
        transcribe(ctx, audio_path, language)
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(CanaryEngine) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jona_types::{ASREngine, DownloadType};

    #[test]
    fn engine_registers_as_asr() {
        // Canary is a speech recognition engine that appears in the ASR model picker.
        let engine = CanaryEngine;
        assert_eq!(engine.engine_id(), "canary");
        assert_eq!(engine.category(), jona_types::EngineCategory::ASR);
    }

    #[test]
    fn user_can_pick_at_least_one_model() {
        let engine = CanaryEngine;
        assert!(!engine.models().is_empty(), "User must be able to choose at least one Canary model");
    }

    #[test]
    fn no_duplicate_models_in_picker() {
        let engine = CanaryEngine;
        let models = engine.models();
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count, "Duplicate models would confuse the user");
    }

    #[test]
    fn all_download_urls_are_secure() {
        let engine = CanaryEngine;
        for model in engine.models() {
            if let DownloadType::MultiFile { files } = &model.download_type {
                for file in files {
                    assert!(file.url.starts_with("https://"),
                        "Model {} file {} has insecure download URL: {}", model.id, file.filename, file.url);
                }
            }
        }
    }

    #[test]
    fn models_report_size_for_download_progress() {
        let engine = CanaryEngine;
        for model in engine.models() {
            assert!(model.size > 0,
                "Model {} reports zero size, download progress UI would be broken", model.id);
        }
    }

    #[test]
    fn canary_supports_multiple_languages() {
        // Canary is a multilingual ASR engine; the user expects to select languages.
        let engine = CanaryEngine;
        let langs = engine.supported_languages();
        assert!(langs.len() > 1, "Canary should support multiple languages for user selection");
    }
}

#[cfg(test)]
mod smoke {
    use std::path::Path;

    #[test]
    #[ignore]
    fn transcribe_real_model() {
        let dir = std::env::var("CANARY_MODEL_DIR").expect("CANARY_MODEL_DIR");
        let wav = std::env::var("CANARY_TEST_WAV").expect("CANARY_TEST_WAV");
        let lang = std::env::var("CANARY_LANG").unwrap_or_else(|_| "fr".into());

        let t_load = std::time::Instant::now();
        let mut ctx = super::load(Path::new(&dir)).expect("load failed");
        eprintln!("LOAD_SECS={:.2}", t_load.elapsed().as_secs_f64());

        let t0 = std::time::Instant::now();
        let r = super::transcribe(&mut ctx, Path::new(&wav), &lang).expect("transcribe failed");
        eprintln!("INFER_SECS={:.3}", t0.elapsed().as_secs_f64());
        eprintln!("TEXT={}", r.text);
        assert!(!r.text.trim().is_empty());
    }
}
