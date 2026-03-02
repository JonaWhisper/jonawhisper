//! Mel spectrogram extraction for NeMo-compatible models (Canary, Parakeet).
//!
//! Config: 16kHz, n_fft=512, win=400 (25ms), hop=160 (10ms), 128 mel bands,
//! Hann window, log power spectrum, per-feature z-normalization.
//! Output shape: [1, 128, n_frames] stored as flat Vec<f32> in row-major order.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::f32::consts::PI;

const N_FFT: usize = 512;
const WIN_LENGTH: usize = 400;
const HOP_LENGTH: usize = 160;
const N_MELS: usize = 128;
const SAMPLE_RATE: f32 = 16000.0;
const LOG_GUARD: f32 = 1e-5;
const NORM_EPS: f32 = 1e-5;

/// Extract 128-channel mel spectrogram from mono 16kHz audio.
/// Returns (features, n_frames) where features is [1, N_MELS, n_frames] flat row-major.
pub fn extract_features(audio: &[f32]) -> (Vec<f32>, usize) {
    if audio.is_empty() {
        return (vec![0.0; N_MELS], 1);
    }

    // Symmetric zero-padding
    let pad = N_FFT / 2;
    let mut padded = vec![0.0f32; audio.len() + 2 * pad];
    padded[pad..pad + audio.len()].copy_from_slice(audio);

    let n_frames = audio.len() / HOP_LENGTH + 1;

    // Hann window
    let mut window = vec![0.0f32; WIN_LENGTH];
    for i in 0..WIN_LENGTH {
        window[i] = 0.5 - 0.5 * (2.0 * PI * i as f32 / (WIN_LENGTH - 1) as f32).cos();
    }

    let n_freqs = N_FFT / 2 + 1;
    let mel_filters = build_mel_filterbank(N_FFT, N_MELS, SAMPLE_RATE);

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(N_FFT);

    // Compute log-mel spectrogram [N_MELS, n_frames]
    let mut mel_spec = vec![0.0f32; N_MELS * n_frames];
    let mut spectrum = vec![Complex::new(0.0f32, 0.0f32); N_FFT];
    let mut power = vec![0.0f32; n_freqs];

    for frame_idx in 0..n_frames {
        let start = frame_idx * HOP_LENGTH;

        // Zero-fill and apply window
        for s in spectrum.iter_mut() {
            *s = Complex::new(0.0, 0.0);
        }
        for i in 0..WIN_LENGTH {
            let sample = if start + i < padded.len() { padded[start + i] } else { 0.0 };
            spectrum[i].re = sample * window[i];
        }

        fft.process(&mut spectrum);

        // Power spectrum
        for i in 0..n_freqs {
            let c = spectrum[i];
            power[i] = c.re * c.re + c.im * c.im;
        }

        // Apply mel filterbank + log
        for m in 0..N_MELS {
            let mut energy = 0.0f32;
            for (k, &w) in mel_filters[m].iter().enumerate() {
                energy += w * power[k];
            }
            mel_spec[m * n_frames + frame_idx] = (energy + LOG_GUARD).ln();
        }
    }

    // Per-feature z-normalization (across time for each mel band)
    let mut features = vec![0.0f32; N_MELS * n_frames];
    for m in 0..N_MELS {
        let row_offset = m * n_frames;

        let mut sum = 0.0f32;
        for t in 0..n_frames {
            sum += mel_spec[row_offset + t];
        }
        let mean = sum / n_frames as f32;

        let mut var = 0.0f32;
        for t in 0..n_frames {
            let diff = mel_spec[row_offset + t] - mean;
            var += diff * diff;
        }
        let std = (var / n_frames as f32).sqrt().max(NORM_EPS);

        for t in 0..n_frames {
            features[row_offset + t] = (mel_spec[row_offset + t] - mean) / std;
        }
    }

    (features, n_frames)
}

/// Build triangular mel filterbank (HTK mel scale).
fn build_mel_filterbank(n_fft: usize, n_mels: usize, sample_rate: f32) -> Vec<Vec<f32>> {
    fn hz_to_mel(hz: f32) -> f32 {
        2595.0 * (1.0 + hz / 700.0).log10()
    }

    fn mel_to_hz(mel: f32) -> f32 {
        700.0 * (10f32.powf(mel / 2595.0) - 1.0)
    }

    let mel_min = hz_to_mel(0.0);
    let mel_max = hz_to_mel(sample_rate / 2.0);

    let mut mel_points = Vec::with_capacity(n_mels + 2);
    for i in 0..(n_mels + 2) {
        let mel = mel_min + (mel_max - mel_min) * i as f32 / (n_mels + 1) as f32;
        mel_points.push(mel_to_hz(mel));
    }

    let freq_bins = n_fft / 2 + 1;
    let freq_bin_width = sample_rate / n_fft as f32;

    let mut filters = vec![vec![0.0f32; freq_bins]; n_mels];
    for m in 0..n_mels {
        let left = mel_points[m];
        let center = mel_points[m + 1];
        let right = mel_points[m + 2];

        for k in 0..freq_bins {
            let freq = k as f32 * freq_bin_width;
            if freq >= left && freq <= center {
                filters[m][k] = (freq - left) / (center - left);
            } else if freq > center && freq <= right {
                filters[m][k] = (right - freq) / (right - center);
            }
        }
    }

    filters
}
