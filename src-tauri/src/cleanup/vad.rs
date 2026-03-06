use std::sync::Mutex;

use ndarray::{Array3, ArrayViewD};
use ort::session::Session;
use ort::value::Tensor;

/// Silero VAD v5 ONNX model, bundled at compile time (~2.3 MB).
const MODEL_BYTES: &[u8] = include_bytes!("../../models/silero_vad.onnx");

const SAMPLE_RATE: i64 = 16_000;
const CHUNK_SIZE: usize = 512;
const CONTEXT_SIZE: usize = 64;
const WINDOW_SIZE: usize = CONTEXT_SIZE + CHUNK_SIZE; // 576
const STATE_DIM: usize = 128;
const DEFAULT_THRESHOLD: f32 = 0.5;

/// Persistent VAD state: ONNX session + recurrent hidden state + context buffer.
struct VadState {
    session: Session,
    /// LSTM hidden state [2, 1, 128]
    state: Array3<f32>,
    /// Last 64 samples from previous chunk (temporal continuity)
    context: Vec<f32>,
}

static VAD: Mutex<Option<VadState>> = Mutex::new(None);

fn init_vad() -> Result<VadState, String> {
    log::info!("Loading Silero VAD model from embedded bytes...");
    let session = Session::builder()
        .map_err(|e| format!("VAD session builder: {e}"))?
        .with_intra_threads(1)
        .map_err(|e| format!("VAD set threads: {e}"))?
        .commit_from_memory(MODEL_BYTES)
        .map_err(|e| format!("VAD load model: {e}"))?;
    log::info!("Silero VAD model loaded");

    Ok(VadState {
        session,
        state: Array3::<f32>::zeros((2, 1, STATE_DIM)),
        context: vec![0.0; CONTEXT_SIZE],
    })
}

/// Acquire the global VAD, run `f`, then reset internal state.
fn with_vad<T>(f: impl FnOnce(&mut VadState) -> Result<T, String>) -> Result<T, String> {
    let mut guard = VAD.lock().map_err(|e| format!("VAD lock poisoned: {e}"))?;
    if guard.is_none() {
        *guard = Some(init_vad()?);
    }
    let vad = guard.as_mut().unwrap();
    let result = f(vad);
    // Reset state for next use
    vad.state.fill(0.0);
    vad.context.fill(0.0);
    result
}

/// Run one chunk (512 samples) through the VAD, returns speech probability.
fn forward_chunk(vad: &mut VadState, chunk: &[f32]) -> Result<f32, String> {
    // Build input: context (64) + chunk (512) = 576 samples
    let mut input = Vec::with_capacity(WINDOW_SIZE);
    input.extend_from_slice(&vad.context);
    input.extend_from_slice(chunk);
    // Pad if chunk was short
    input.resize(WINDOW_SIZE, 0.0);

    // Update context with last 64 samples of this chunk
    let src = if chunk.len() >= CONTEXT_SIZE {
        &chunk[chunk.len() - CONTEXT_SIZE..]
    } else {
        // Short chunk: shift context and append
        let keep = CONTEXT_SIZE - chunk.len();
        let mut new_ctx = vad.context[CONTEXT_SIZE - keep..].to_vec();
        new_ctx.extend_from_slice(chunk);
        vad.context = new_ctx;
        &[] as &[f32]
    };
    if !src.is_empty() {
        vad.context.clear();
        vad.context.extend_from_slice(src);
    }

    // Tensors
    let input_tensor = Tensor::from_array(([1usize, WINDOW_SIZE], input))
        .map_err(|e| format!("VAD input tensor: {e}"))?;
    let state_tensor = Tensor::from_array(vad.state.clone())
        .map_err(|e| format!("VAD state tensor: {e}"))?;
    let sr_tensor = Tensor::from_array(([1usize], vec![SAMPLE_RATE]))
        .map_err(|e| format!("VAD sr tensor: {e}"))?;

    let outputs = vad
        .session
        .run(ort::inputs![
            "input" => input_tensor,
            "state" => state_tensor,
            "sr" => sr_tensor,
        ])
        .map_err(|e| format!("VAD inference: {e}"))?;

    // Extract probability
    let (_, prob_data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("VAD extract output: {e}"))?;
    let prob = prob_data.first().copied().unwrap_or(0.0);

    // Update hidden state from output
    let (state_shape, state_data) = outputs[1]
        .try_extract_tensor::<f32>()
        .map_err(|e| format!("VAD extract state: {e}"))?;
    let state_view = ArrayViewD::from_shape(
        state_shape.iter().map(|&d| d as usize).collect::<Vec<_>>(),
        state_data,
    )
    .map_err(|e| format!("VAD reshape state: {e}"))?;
    if let Some(s3) = state_view.into_dimensionality::<ndarray::Ix3>().ok() {
        vad.state.assign(&s3);
    }

    Ok(prob)
}

/// Result of a single-pass VAD analysis.
pub enum VadAnalysis {
    /// No speech detected anywhere.
    NoSpeech,
    /// Speech found; (start, end) sample indices with one-chunk margin.
    Speech { start: usize, end: usize },
}

/// Analyze audio in a single pass: detect speech presence AND find bounds.
/// Falls back to full range on error (never lose the dictation).
pub fn analyze(audio: &[f32]) -> VadAnalysis {
    match analyze_inner(audio) {
        Ok(result) => result,
        Err(e) => {
            log::warn!("VAD analyze error, assuming full speech: {e}");
            VadAnalysis::Speech { start: 0, end: audio.len() }
        }
    }
}

fn analyze_inner(audio: &[f32]) -> Result<VadAnalysis, String> {
    with_vad(|vad| {
        let mut first_speech: Option<usize> = None;
        let mut last_speech: Option<usize> = None;

        for (i, chunk) in audio.chunks(CHUNK_SIZE).enumerate() {
            if forward_chunk(vad, chunk)? > DEFAULT_THRESHOLD {
                if first_speech.is_none() {
                    first_speech = Some(i);
                }
                last_speech = Some(i);
            }
        }

        match (first_speech, last_speech) {
            (Some(first), Some(last)) => {
                let start = first.saturating_sub(1) * CHUNK_SIZE;
                let end = ((last + 2) * CHUNK_SIZE).min(audio.len());
                Ok(VadAnalysis::Speech { start, end })
            }
            _ => Ok(VadAnalysis::NoSpeech),
        }
    })
}
