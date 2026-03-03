use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use hound::{WavSpec, WavWriter};
use rustfft::{num_complex::Complex, FftPlanner};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::engines::EngineError;

const SAMPLE_RATE: u32 = 16000;
const NUM_BANDS: usize = 12;
const FFT_SIZE: usize = 1024;
const SPECTRUM_SMOOTHING: f32 = 0.55; // new data weight (old = 1 - this)

/// List input devices from CoreAudio, filtered to only those cpal can use for recording.
pub fn list_usable_devices() -> Vec<crate::platform::audio_devices::AudioDevice> {
    let host = cpal::default_host();
    let cpal_names: Vec<String> = host
        .input_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();

    crate::platform::audio_devices::list_input_devices()
        .into_iter()
        .filter(|d| cpal_names.iter().any(|n| n == &d.name))
        .collect()
}

pub struct AudioRecorder {
    stream: Option<cpal::Stream>,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<std::fs::File>>>>>,
    current_file: Arc<Mutex<Option<PathBuf>>>,
    spectrum: Arc<Mutex<Vec<f32>>>,
    fft_buffer: Arc<Mutex<Vec<f32>>>,
    stream_error: Arc<AtomicBool>,
}

impl AudioRecorder {
    pub fn new(stream_error: Arc<AtomicBool>) -> Self {
        Self {
            stream: None,
            writer: Arc::new(Mutex::new(None)),
            current_file: Arc::new(Mutex::new(None)),
            spectrum: Arc::new(Mutex::new(vec![0.0; NUM_BANDS])),
            fft_buffer: Arc::new(Mutex::new(Vec::with_capacity(FFT_SIZE))),
            stream_error,
        }
    }

    pub fn start_recording(&mut self, device_uid: Option<&str>) -> bool {
        let host = cpal::default_host();

        // Resolve CoreAudio UID to device name, then find the matching cpal device
        let device = if let Some(uid) = device_uid {
            let device_name = crate::platform::audio_devices::list_input_devices()
                .into_iter()
                .find(|d| d.uid == uid)
                .map(|d| d.name);

            if let Some(name) = device_name {
                host.input_devices()
                    .ok()
                    .and_then(|mut devices| devices.find(|d| d.name().ok().as_deref() == Some(&name)))
                    .or_else(|| {
                        log::warn!("CoreAudio device '{}' not found in cpal, using default", name);
                        host.default_input_device()
                    })
            } else {
                log::warn!("No CoreAudio device with UID '{}', using default", uid);
                host.default_input_device()
            }
        } else {
            host.default_input_device()
        };

        let device = match device {
            Some(d) => d,
            None => {
                log::error!("No input device available");
                return false;
            }
        };

        // Try 16kHz mono first, fall back to device default config
        let preferred = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };

        let default_cfg = device.default_input_config().ok();
        let (config, native_channels, native_rate) = if device
            .supported_input_configs()
            .ok()
            .map(|configs| {
                configs.into_iter().any(|c| {
                    c.channels() >= 1
                        && c.min_sample_rate().0 <= SAMPLE_RATE
                        && c.max_sample_rate().0 >= SAMPLE_RATE
                })
            })
            .unwrap_or(false)
        {
            (preferred, 1u16, SAMPLE_RATE)
        } else if let Some(ref cfg) = default_cfg {
            let sc = cpal::StreamConfig {
                channels: cfg.channels(),
                sample_rate: cfg.sample_rate(),
                buffer_size: cpal::BufferSize::Default,
            };
            log::info!(
                "Using device native config: {}Hz {}ch (will resample to 16kHz mono)",
                cfg.sample_rate().0,
                cfg.channels()
            );
            (sc, cfg.channels(), cfg.sample_rate().0)
        } else {
            log::error!("No supported input configuration found");
            return false;
        };

        // Create WAV file — always write 16kHz mono for Whisper
        let tmp_dir = std::env::temp_dir();
        let filename = format!(
            "jona_whisper_{}.wav",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let filepath = tmp_dir.join(&filename);

        let spec = WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let writer = match WavWriter::create(&filepath, spec) {
            Ok(w) => w,
            Err(e) => {
                log::error!("Failed to create WAV file: {}", e);
                return false;
            }
        };

        *self.current_file.lock().unwrap() = Some(filepath);
        *self.writer.lock().unwrap() = Some(writer);
        *self.spectrum.lock().unwrap() = vec![0.0; NUM_BANDS];
        *self.fft_buffer.lock().unwrap() = Vec::with_capacity(FFT_SIZE);
        self.stream_error.store(false, Ordering::SeqCst);

        let writer_clone = Arc::clone(&self.writer);
        let fft_buffer_clone = Arc::clone(&self.fft_buffer);
        let spectrum_clone = Arc::clone(&self.spectrum);

        let sample_format = default_cfg
            .map(|c| c.sample_format())
            .unwrap_or(SampleFormat::F32);

        let channels = native_channels;
        let rate = native_rate;

        let stream = match sample_format {
            SampleFormat::F32 => {
                let error_flag = Arc::clone(&self.stream_error);
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mono = mix_to_mono(data, channels);
                        let resampled = resample(&mono, rate, SAMPLE_RATE);
                        process_samples(&resampled, &writer_clone, &fft_buffer_clone, &spectrum_clone);
                    },
                    move |err| {
                        log::error!("Audio stream error: {}", err);
                        error_flag.store(true, Ordering::SeqCst);
                    },
                    None,
                )
            }
            SampleFormat::I16 => {
                let writer_clone2 = Arc::clone(&self.writer);
                let fft_buffer_clone2 = Arc::clone(&self.fft_buffer);
                let spectrum_clone2 = Arc::clone(&self.spectrum);
                let error_flag = Arc::clone(&self.stream_error);
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let float_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                        let mono = mix_to_mono(&float_data, channels);
                        let resampled = resample(&mono, rate, SAMPLE_RATE);
                        process_samples(&resampled, &writer_clone2, &fft_buffer_clone2, &spectrum_clone2);
                    },
                    move |err| {
                        log::error!("Audio stream error: {}", err);
                        error_flag.store(true, Ordering::SeqCst);
                    },
                    None,
                )
            }
            _ => {
                log::error!("Unsupported sample format: {:?}", sample_format);
                return false;
            }
        };

        match stream {
            Ok(s) => {
                if let Err(e) = s.play() {
                    log::error!("Failed to play stream: {}", e);
                    return false;
                }
                self.stream = Some(s);
                true
            }
            Err(e) => {
                log::error!("Failed to build input stream: {}", e);
                false
            }
        }
    }

    pub fn stop_recording(&mut self) -> Option<PathBuf> {
        // Drop the stream first to stop recording
        self.stream = None;

        // Finalize the WAV writer
        if let Some(writer) = self.writer.lock().unwrap().take() {
            let _ = writer.finalize();
        }

        self.current_file.lock().unwrap().take()
    }

    pub fn get_spectrum(&self) -> Vec<f32> {
        self.spectrum.lock().unwrap().clone()
    }
}

/// Mix multi-channel audio to mono by averaging channels.
fn mix_to_mono(data: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return data.to_vec();
    }
    let ch = channels as usize;
    data.chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// Simple linear resampling from src_rate to dst_rate.
fn resample(data: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if src_rate == dst_rate || data.is_empty() {
        return data.to_vec();
    }
    let ratio = src_rate as f64 / dst_rate as f64;
    let out_len = (data.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = src_pos - idx as f64;
        let s0 = data[idx];
        let s1 = if idx + 1 < data.len() { data[idx + 1] } else { s0 };
        out.push(s0 + (s1 - s0) * frac as f32);
    }
    out
}

fn process_samples(
    data: &[f32],
    writer: &Mutex<Option<WavWriter<BufWriter<std::fs::File>>>>,
    fft_buffer: &Mutex<Vec<f32>>,
    spectrum: &Mutex<Vec<f32>>,
) {
    // Write to WAV
    if let Some(ref mut w) = *writer.lock().unwrap() {
        for &sample in data {
            let s16 = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            let _ = w.write_sample(s16);
        }
    }

    // Accumulate FFT buffer
    let mut buf = fft_buffer.lock().unwrap();
    buf.extend_from_slice(data);

    if buf.len() >= FFT_SIZE {
        let samples: Vec<f32> = buf.drain(..FFT_SIZE).collect();
        let new_spectrum = compute_spectrum(&samples);

        let mut spec = spectrum.lock().unwrap();
        let old_weight = 1.0 - SPECTRUM_SMOOTHING;
        for (s, &ns) in spec.iter_mut().zip(new_spectrum.iter()) {
            *s = *s * old_weight + ns * SPECTRUM_SMOOTHING;
        }
    }
}

fn compute_spectrum(samples: &[f32]) -> Vec<f32> {
    use std::sync::LazyLock;
    static FFT: LazyLock<std::sync::Arc<dyn rustfft::Fft<f32>>> =
        LazyLock::new(|| FftPlanner::new().plan_fft_forward(FFT_SIZE));
    let fft = &*FFT;

    let mut buffer: Vec<Complex<f32>> = samples
        .iter()
        .map(|&s| Complex { re: s, im: 0.0 })
        .collect();

    fft.process(&mut buffer);

    // Convert to magnitude spectrum (only first half)
    let half = FFT_SIZE / 2;
    let magnitudes: Vec<f32> = buffer[..half]
        .iter()
        .map(|c| (c.re * c.re + c.im * c.im).sqrt())
        .collect();

    // Map to logarithmic bands
    let mut bands = [0.0f32; NUM_BANDS];
    let min_freq = 80.0f32;
    let max_freq = (SAMPLE_RATE as f32) / 2.0;
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();

    for (i, band) in bands.iter_mut().enumerate() {
        let lo = ((log_min + (log_max - log_min) * i as f32 / NUM_BANDS as f32).exp()
            / max_freq * half as f32) as usize;
        let hi = ((log_min + (log_max - log_min) * (i + 1) as f32 / NUM_BANDS as f32).exp()
            / max_freq * half as f32) as usize;

        let lo = lo.min(half - 1);
        let hi = hi.min(half).max(lo + 1);

        let sum: f32 = magnitudes[lo..hi].iter().sum();
        let avg = sum / (hi - lo) as f32;

        // Logarithmic (dB) normalization: -50dB..0dB → 0..1, gamma 0.8 for airy mid-range
        let db = 20.0 * avg.max(1e-10).log10();
        let normalized = ((db + 50.0) / 50.0).clamp(0.0, 1.0);
        *band = normalized.powf(0.8);
    }

    // Reorder bands symmetrically (center outward) like the Swift version
    let mut reordered = [0.0f32; NUM_BANDS];
    let mid = NUM_BANDS / 2;
    for (i, val) in reordered.iter_mut().enumerate() {
        let src = if i % 2 == 0 { mid - 1 - i / 2 } else { mid + i / 2 };
        if src < NUM_BANDS {
            *val = bands[src];
        }
    }

    reordered.to_vec()
}

/// Read a WAV file and convert to f32 mono samples.
pub(crate) fn read_wav_f32(path: &Path) -> Result<Vec<f32>, EngineError> {
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

    // Convert to mono by averaging channels
    if channels > 1 {
        Ok(samples_f32
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect())
    } else {
        Ok(samples_f32)
    }
}
