use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use jona_types::{
    ASREngine, ASRModel, DownloadFile, DownloadType, EngineError, EngineRegistration,
    GpuMode, Language, TranscriptionResult,
};
use std::any::Any;
use std::path::Path;

const REPO_URL: &str = "https://huggingface.co/kyutai/stt-1b-en_fr-candle/resolve/main";
const CONFIG_FILENAME: &str = "config.json";
const WEIGHTS_FILENAME: &str = "model.safetensors";
const MODEL_FILES: [(&str, u64); 4] = [
    (CONFIG_FILENAME, 1_315),
    (WEIGHTS_FILENAME, 1_978_522_200),
    ("mimi-pytorch-e351c8d8@125.safetensors", 384_644_900),
    ("tokenizer_en_fr_audio_8000.model", 120_378),
];

const RECORDING_RATE: usize = 16_000;
const SAMPLE_RATE: usize = 24_000;
/// Mimi consumes 80 ms frames, 12.5 of them per second.
const FRAME_SAMPLES: usize = 1920;
const FRAMES_PER_SECOND: f64 = 12.5;

#[derive(serde::Deserialize)]
struct SttConfig {
    audio_silence_prefix_seconds: f64,
    audio_delay_seconds: f64,
}

#[derive(serde::Deserialize)]
struct Config {
    mimi_name: String,
    tokenizer_name: String,
    card: usize,
    text_card: usize,
    dim: usize,
    n_q: usize,
    context: usize,
    max_period: f64,
    num_heads: usize,
    num_layers: usize,
    causal: bool,
    stt_config: SttConfig,
}

impl Config {
    fn model_config(&self) -> moshi::lm::Config {
        let transformer = moshi::transformer::Config {
            d_model: self.dim,
            num_heads: self.num_heads,
            num_layers: self.num_layers,
            dim_feedforward: self.dim * 4,
            causal: self.causal,
            norm_first: true,
            bias_ff: false,
            bias_attn: false,
            layer_scale: None,
            context: self.context,
            max_period: self.max_period as usize,
            use_conv_block: false,
            use_conv_bias: true,
            cross_attention: None,
            gating: Some(candle_nn::Activation::Silu),
            norm: moshi::NormType::RmsNorm,
            positional_embedding: moshi::transformer::PositionalEmbedding::Rope,
            conv_layout: false,
            conv_kernel_size: 3,
            kv_repeat: 1,
            max_seq_len: 4096 * 4,
            shared_cross_attn: false,
        };
        moshi::lm::Config {
            transformer,
            depformer: None,
            audio_vocab_size: self.card + 1,
            text_in_vocab_size: self.text_card + 1,
            text_out_vocab_size: self.text_card,
            audio_codebooks: self.n_q,
            conditioners: Default::default(),
            extra_heads: None,
        }
    }
}

pub struct KyutaiContext {
    state: moshi::asr::State,
    pieces: Vec<String>,
    config: Config,
    device: Device,
}

fn failed<E: std::fmt::Display>(what: &'static str) -> impl FnOnce(E) -> EngineError {
    move |e| EngineError::LaunchFailed(format!("Kyutai {what}: {e}"))
}

fn select_device(gpu_mode: GpuMode) -> Device {
    if gpu_mode != GpuMode::Cpu && candle_core::utils::metal_is_available() {
        if let Ok(device) = Device::new_metal(0) {
            return device;
        }
    }
    Device::Cpu
}

pub fn load(model_dir: &Path, gpu_mode: GpuMode) -> Result<KyutaiContext, EngineError> {
    let device = select_device(gpu_mode);

    let config = std::fs::read_to_string(model_dir.join(CONFIG_FILENAME)).map_err(failed("config"))?;
    let config: Config = serde_json::from_str(&config).map_err(failed("config"))?;

    let pieces = jona_engines::sentencepiece::read_model(&model_dir.join(&config.tokenizer_name))
        .map_err(EngineError::LaunchFailed)?
        .pieces
        .into_iter()
        .map(|p| p.piece.unwrap_or_default())
        .collect();

    let weights = model_dir.join(WEIGHTS_FILENAME);
    let dtype = device.bf16_default_to_f32();
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights], dtype, &device) }
        .map_err(failed("weights"))?;
    let lm = moshi::lm::LmModel::new(&config.model_config(), moshi::nn::MaybeQuantizedVarBuilder::Real(vb))
        .map_err(failed("model"))?;

    let mimi_path = model_dir.join(&config.mimi_name);
    let mimi_path = mimi_path.to_str().ok_or_else(|| failed("mimi")("non UTF-8 path"))?;
    let audio_tokenizer = moshi::mimi::load(mimi_path, Some(config.n_q), &device).map_err(failed("mimi"))?;

    let delay_in_tokens = (config.stt_config.audio_delay_seconds * FRAMES_PER_SECOND) as usize;
    let state = moshi::asr::State::new(1, delay_in_tokens, 0., audio_tokenizer, lm).map_err(failed("state"))?;

    Ok(KyutaiContext { state, pieces, config, device })
}

pub fn transcribe(ctx: &mut KyutaiContext, audio_path: &Path, _language: &str) -> Result<TranscriptionResult, EngineError> {
    let audio = jona_engines::audio::read_wav_f32(audio_path)?;

    let stt = &ctx.config.stt_config;
    let prefix = (stt.audio_silence_prefix_seconds * SAMPLE_RATE as f64) as usize;
    // Text comes out audio_delay_seconds after its audio: without this tail the last words are lost.
    let suffix = (stt.audio_delay_seconds * SAMPLE_RATE as f64) as usize + SAMPLE_RATE;
    let mut pcm = vec![0.0; prefix];
    pcm.extend(resample_recording(&audio).map_err(failed("resampling"))?);
    pcm.resize(pcm.len() + suffix, 0.0);

    ctx.state.reset().map_err(failed("reset"))?;

    let mut words = Vec::new();
    for frame in pcm.chunks(FRAME_SAMPLES) {
        let frame = Tensor::new(frame, &ctx.device)
            .and_then(|t| t.reshape((1, 1, ())))
            .map_err(failed("frame"))?;
        let messages = ctx.state.step_pcm(frame, None, &().into(), |_, _, _| ()).map_err(failed("step"))?;
        for message in messages {
            if let moshi::asr::AsrMsg::Word { tokens, .. } = message {
                words.push(decode_pieces(&ctx.pieces, &tokens));
            }
        }
    }

    Ok(TranscriptionResult::text_only(words.join(" ")))
}

/// Same resampler and settings as Kyutai's reference code. The model is that sensitive:
/// a windowed sinc changed 37 of 40 French transcriptions, one of them to Persian script.
fn resample_recording(input: &[f32]) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    use rubato::Resampler;

    let mut output = Vec::with_capacity(input.len() * 3 / 2 + 1024);
    let mut resampler = rubato::FftFixedInOut::<f32>::new(RECORDING_RATE, SAMPLE_RATE, 1024, 1)?;
    let mut buffer = resampler.output_buffer_allocate(true);
    let mut position = 0;
    while position + resampler.input_frames_next() < input.len() {
        let (consumed, produced) = resampler.process_into_buffer(&[&input[position..]], &mut buffer, None)?;
        position += consumed;
        output.extend_from_slice(&buffer[0][..produced]);
    }
    if position < input.len() {
        let (_, produced) = resampler.process_partial_into_buffer(Some(&[&input[position..]]), &mut buffer, None)?;
        output.extend_from_slice(&buffer[0][..produced]);
    }
    Ok(output)
}

/// SentencePiece decoding, for the three kinds of pieces this vocabulary holds:
/// `<0xNN>` bytes (256 of them), `<s>`-style controls, and text with `▁` for spaces.
fn decode_pieces(pieces: &[String], tokens: &[u32]) -> String {
    let mut bytes = Vec::new();
    for piece in tokens.iter().filter_map(|&token| pieces.get(token as usize)) {
        let hex = piece.strip_prefix("<0x").and_then(|p| p.strip_suffix('>'));
        match hex.map(|hex| u8::from_str_radix(hex, 16)) {
            Some(Ok(byte)) => bytes.push(byte),
            _ if piece.starts_with('<') && piece.ends_with('>') => {}
            _ => bytes.extend_from_slice(piece.as_bytes()),
        }
    }
    String::from_utf8_lossy(&bytes).replace('\u{2581}', " ").trim().to_string()
}

pub struct KyutaiEngine;

impl ASREngine for KyutaiEngine {
    fn engine_id(&self) -> &str { "kyutai" }
    fn display_name(&self) -> &str { "Kyutai" }

    fn models(&self) -> Vec<ASRModel> {
        vec![ASRModel {
            id: "kyutai:stt-1b-en_fr".into(),
            engine_id: "kyutai".into(),
            label: "Kyutai STT".into(),
            quantization: Some("BF16".into()),
            filename: "stt-1b-en_fr".into(),
            url: String::new(),
            size: MODEL_FILES.iter().map(|(_, size)| size).sum(),
            storage_dir: jona_types::engine_storage_dir("kyutai"),
            download_type: DownloadType::MultiFile {
                files: MODEL_FILES
                    .iter()
                    .map(|(filename, size)| DownloadFile {
                        filename: (*filename).into(),
                        url: format!("{REPO_URL}/{filename}"),
                        size: *size,
                    })
                    .collect(),
            },
            download_marker: Some(".complete".into()),
            // Measured on Metal, 40 French clips: 5.8x real time, 2.68 GB peak footprint.
            rtf: Some(0.17),
            ram: Some(2_700_000_000),
            params: Some(1.0),
            lang_codes: Some(vec!["en".into(), "fr".into()]),
            runtime: Some("candle".into()),
            ..Default::default()
        }]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "en".into(), label: "English".into() },
            Language { code: "fr".into(), label: "Fran\u{00e7}ais".into() },
        ]
    }

    fn description(&self) -> &str {
        "Kyutai STT, built on Moshi. English and French, punctuation and capitalization included."
    }

    fn context_key(&self, model: &ASRModel, gpu_mode: GpuMode) -> String {
        format!("{}:{:?}", model.id, gpu_mode)
    }

    fn create_context(&self, model: &ASRModel, gpu_mode: GpuMode)
        -> Result<Box<dyn Any + Send>, EngineError>
    {
        let ctx = load(&model.local_path(), gpu_mode)?;
        Ok(Box::new(ctx))
    }

    fn transcribe(&self, ctx: &mut dyn Any, audio_path: &Path, language: &str)
        -> Result<TranscriptionResult, EngineError>
    {
        let ctx = ctx.downcast_mut::<KyutaiContext>()
            .ok_or_else(|| EngineError::LaunchFailed("Invalid kyutai context".into()))?;
        transcribe(ctx, audio_path, language)
    }
}

inventory::submit! {
    EngineRegistration { factory: || Box::new(KyutaiEngine) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(items: &[&str]) -> Vec<String> {
        items.iter().map(|p| (*p).to_string()).collect()
    }

    fn sine(rate: usize, count: usize) -> Vec<f32> {
        (0..count)
            .map(|n| (2.0 * std::f64::consts::PI * 440.0 * n as f64 / rate as f64).sin() as f32)
            .collect()
    }

    #[test]
    fn engine_registers_as_asr() {
        assert_eq!(KyutaiEngine.engine_id(), "kyutai");
        assert_eq!(KyutaiEngine.category(), jona_types::EngineCategory::ASR);
    }

    #[test]
    fn download_is_secure_and_reports_its_size() {
        let model = KyutaiEngine.models().remove(0);
        let DownloadType::MultiFile { files } = &model.download_type else { panic!("multi-file expected") };
        assert!(files.iter().all(|f| f.url.starts_with("https://")));
        assert_eq!(model.size, files.iter().map(|f| f.size).sum::<u64>());
    }

    #[test]
    fn supports_french_and_english() {
        let languages = KyutaiEngine.supported_languages();
        let codes: Vec<&str> = languages.iter().map(|l| l.code.as_str()).collect();
        assert_eq!(codes, ["en", "fr"]);
    }

    #[test]
    fn decode_turns_metaspace_into_spaces() {
        let vocab = pieces(&["<unk>", "\u{2581}bon", "jour", ","]);
        assert_eq!(decode_pieces(&vocab, &[1, 2, 3]), "bonjour,");
    }

    #[test]
    fn decode_reassembles_multibyte_characters_from_byte_pieces() {
        let vocab = pieces(&["<0xC3>", "<0xA9>", "t"]);
        assert_eq!(decode_pieces(&vocab, &[0, 1, 2]), "\u{00e9}t");
    }

    #[test]
    fn decode_drops_control_pieces_and_unknown_ids() {
        let vocab = pieces(&["<s>", "\u{2581}oui", "</s>"]);
        assert_eq!(decode_pieces(&vocab, &[0, 1, 2, 99]), "oui");
    }

    #[test]
    fn resampling_yields_three_samples_for_two() {
        let length = resample_recording(&vec![0.0; RECORDING_RATE]).unwrap().len();
        assert!((SAMPLE_RATE..SAMPLE_RATE + 2048).contains(&length), "got {length}");
    }

    #[test]
    fn resampling_nothing_yields_nothing() {
        assert!(resample_recording(&[]).unwrap().is_empty());
    }

    #[test]
    fn resampling_keeps_the_pitch_and_level_of_a_tone() {
        let output = resample_recording(&sine(RECORDING_RATE, RECORDING_RATE)).unwrap();
        let middle = &output[2_000..22_000];

        let crossings = middle.windows(2).filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0).count() as f64;
        let hertz = crossings / (middle.len() as f64 / SAMPLE_RATE as f64);
        assert!((hertz - 440.0).abs() < 2.0, "got {hertz} Hz");

        let rms = (middle.iter().map(|s| s * s).sum::<f32>() / middle.len() as f32).sqrt();
        assert!((rms - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01, "got RMS {rms}");
    }
}
