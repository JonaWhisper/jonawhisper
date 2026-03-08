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
const NORM_EPS: f32 = 1e-5;

/// Mel scale variant for filterbank construction.
#[derive(Clone, Copy)]
pub enum MelScale {
    /// HTK formula: mel = 2595 * log10(1 + hz/700). Used by Canary.
    HTK,
    /// Slaney/librosa formula: linear below 1kHz, logarithmic above. Used by Parakeet.
    Slaney,
}

/// Configuration for mel spectrogram extraction.
#[derive(Clone, Copy)]
pub struct MelConfig {
    pub mel_scale: MelScale,
    /// Pre-emphasis coefficient. None = no pre-emphasis.
    pub preemphasis: Option<f32>,
    /// Log guard value added before ln(). NeMo uses 2^-24 for TDT, 1e-5 for Canary.
    pub log_guard: f32,
    /// Use Bessel's correction (N-1) for variance normalization.
    pub bessel_correction: bool,
    /// Use Slaney normalization (area normalization) for mel filterbank.
    pub slaney_norm: bool,
}

/// Canary config: HTK mel scale, no pre-emphasis.
pub const CANARY_CONFIG: MelConfig = MelConfig {
    mel_scale: MelScale::HTK,
    preemphasis: None,
    log_guard: 1e-5,
    bessel_correction: false,
    slaney_norm: false,
};

/// Parakeet-TDT config: Slaney mel scale, 0.97 pre-emphasis, NeMo log guard.
pub const PARAKEET_CONFIG: MelConfig = MelConfig {
    mel_scale: MelScale::Slaney,
    preemphasis: Some(0.97),
    log_guard: 5.960_465e-8, // 2^-24
    bessel_correction: true,
    slaney_norm: true,
};

/// Extract 128-channel mel spectrogram from mono 16kHz audio (Canary defaults).
/// Returns (features, n_frames) where features is [1, N_MELS, n_frames] flat row-major.
pub fn extract_features(audio: &[f32]) -> (Vec<f32>, usize) {
    extract_features_with_config(audio, &CANARY_CONFIG)
}

/// Extract mel spectrogram with configurable mel scale and pre-emphasis.
pub fn extract_features_with_config(audio: &[f32], config: &MelConfig) -> (Vec<f32>, usize) {
    if audio.is_empty() {
        return (vec![0.0; N_MELS], 1);
    }

    // Apply pre-emphasis if configured
    let processed;
    let samples = if let Some(coef) = config.preemphasis {
        processed = apply_preemphasis(audio, coef);
        &processed
    } else {
        audio
    };

    // Symmetric zero-padding
    let pad = N_FFT / 2;
    let mut padded = vec![0.0f32; samples.len() + 2 * pad];
    padded[pad..pad + samples.len()].copy_from_slice(samples);

    let n_frames = samples.len() / HOP_LENGTH + 1;

    // Hann window
    let window: Vec<f32> = (0..WIN_LENGTH)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (WIN_LENGTH - 1) as f32).cos())
        .collect();

    let n_freqs = N_FFT / 2 + 1;
    let mel_filters = build_mel_filterbank(N_FFT, N_MELS, SAMPLE_RATE, config.mel_scale, config.slaney_norm);

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
            mel_spec[m * n_frames + frame_idx] = (energy + config.log_guard).ln();
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
        // Bessel's correction: divide by (N-1) instead of N
        let denom = if config.bessel_correction && n_frames > 1 {
            (n_frames - 1) as f32
        } else {
            n_frames as f32
        };
        let std = (var / denom).sqrt().max(NORM_EPS);

        for t in 0..n_frames {
            features[row_offset + t] = (mel_spec[row_offset + t] - mean) / std;
        }
    }

    (features, n_frames)
}

/// Apply pre-emphasis filter: y[i] = x[i] - coef * x[i-1].
fn apply_preemphasis(audio: &[f32], coef: f32) -> Vec<f32> {
    let mut result = Vec::with_capacity(audio.len());
    if audio.is_empty() {
        return result;
    }
    result.push(audio[0]);
    for i in 1..audio.len() {
        result.push(audio[i] - coef * audio[i - 1]);
    }
    result
}

/// Build triangular mel filterbank with configurable mel scale.
fn build_mel_filterbank(n_fft: usize, n_mels: usize, sample_rate: f32, scale: MelScale, slaney_norm: bool) -> Vec<Vec<f32>> {
    let freq_bins = n_fft / 2 + 1;
    let freq_bin_width = sample_rate / n_fft as f32;

    let hz_to_mel: Box<dyn Fn(f32) -> f32> = match scale {
        MelScale::HTK => Box::new(|hz: f32| 2595.0 * (1.0 + hz / 700.0).log10()),
        MelScale::Slaney => Box::new(|hz: f32| {
            const F_SP: f32 = 200.0 / 3.0;
            const MIN_LOG_HZ: f32 = 1000.0;
            const MIN_LOG_MEL: f32 = MIN_LOG_HZ / F_SP;
            const LOG_STEP: f32 = 0.06875178; // ln(6.4) / 27
            if hz < MIN_LOG_HZ { hz / F_SP }
            else { MIN_LOG_MEL + (hz / MIN_LOG_HZ).ln() / LOG_STEP }
        }),
    };

    let mel_to_hz: Box<dyn Fn(f32) -> f32> = match scale {
        MelScale::HTK => Box::new(|mel: f32| 700.0 * (10f32.powf(mel / 2595.0) - 1.0)),
        MelScale::Slaney => Box::new(|mel: f32| {
            const F_SP: f32 = 200.0 / 3.0;
            const MIN_LOG_HZ: f32 = 1000.0;
            const MIN_LOG_MEL: f32 = MIN_LOG_HZ / F_SP;
            const LOG_STEP: f32 = 0.06875178;
            if mel < MIN_LOG_MEL { mel * F_SP }
            else { MIN_LOG_HZ * ((mel - MIN_LOG_MEL) * LOG_STEP).exp() }
        }),
    };

    let mel_min = hz_to_mel(0.0);
    let mel_max = hz_to_mel(sample_rate / 2.0);

    let mut mel_points = Vec::with_capacity(n_mels + 2);
    for i in 0..(n_mels + 2) {
        let mel = mel_min + (mel_max - mel_min) * i as f32 / (n_mels + 1) as f32;
        mel_points.push(mel_to_hz(mel));
    }

    let mut filters = vec![vec![0.0f32; freq_bins]; n_mels];
    for m in 0..n_mels {
        let left = mel_points[m];
        let center = mel_points[m + 1];
        let right = mel_points[m + 2];

        for (k, filter_val) in filters[m].iter_mut().enumerate().take(freq_bins) {
            let freq = k as f32 * freq_bin_width;
            if freq >= left && freq <= center {
                *filter_val = (freq - left) / (center - left);
            } else if freq > center && freq <= right {
                *filter_val = (right - freq) / (right - center);
            }
        }

        // Slaney normalization: area normalization = 2 / (right - left)
        if slaney_norm {
            let width = right - left;
            if width > 0.0 {
                let enorm = 2.0 / width;
                for filter_val in filters[m].iter_mut().take(freq_bins) {
                    *filter_val *= enorm;
                }
            }
        }
    }

    filters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mel_config_canary_defaults() {
        assert!(matches!(CANARY_CONFIG.mel_scale, MelScale::HTK));
        assert!(CANARY_CONFIG.preemphasis.is_none());
        assert!(!CANARY_CONFIG.bessel_correction);
        assert!(!CANARY_CONFIG.slaney_norm);
    }

    #[test]
    fn mel_config_parakeet_defaults() {
        assert!(matches!(PARAKEET_CONFIG.mel_scale, MelScale::Slaney));
        assert_eq!(PARAKEET_CONFIG.preemphasis, Some(0.97));
        assert!(PARAKEET_CONFIG.bessel_correction);
        assert!(PARAKEET_CONFIG.slaney_norm);
    }

    #[test]
    fn extract_features_empty_audio() {
        let (features, n_frames) = extract_features(&[]);
        assert_eq!(n_frames, 1);
        assert_eq!(features.len(), N_MELS);
        // All zeros for empty input
        for &v in &features {
            assert!((v - 0.0).abs() < 1e-5);
        }
    }

    #[test]
    fn extract_features_output_shape() {
        // 16000 samples = 1 second at 16kHz
        let audio = vec![0.0f32; 16000];
        let (features, n_frames) = extract_features(&audio);
        // n_frames = samples / HOP_LENGTH + 1 = 16000/160 + 1 = 101
        assert_eq!(n_frames, 101);
        assert_eq!(features.len(), N_MELS * n_frames);
    }

    #[test]
    fn extract_features_with_config_parakeet_shape() {
        let audio = vec![0.0f32; 16000];
        let (features, n_frames) = extract_features_with_config(&audio, &PARAKEET_CONFIG);
        assert_eq!(n_frames, 101);
        assert_eq!(features.len(), N_MELS * n_frames);
    }

    #[test]
    fn extract_features_normalized() {
        // A sine wave should produce non-trivial features.
        // Use enough audio to have many frames for meaningful statistics.
        let n = 48000; // 3 seconds
        let audio: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 16000.0).sin() * 0.5)
            .collect();
        let (features, n_frames) = extract_features(&audio);
        assert!(n_frames > 10);

        // Check z-normalization: mean should be near zero for bands with variance.
        // Some bands (especially edge bands) may have constant values after log,
        // so we check that MOST bands have near-zero mean.
        let mut near_zero_count = 0;
        for m in 0..N_MELS {
            let mut sum = 0.0f64;
            for t in 0..n_frames {
                sum += features[m * n_frames + t] as f64;
            }
            let mean = sum / n_frames as f64;
            if mean.abs() < 0.1 {
                near_zero_count += 1;
            }
        }
        assert!(near_zero_count > N_MELS / 2,
            "Expected most mel bands to have near-zero mean after normalization, got {}/{}", near_zero_count, N_MELS);
    }

    #[test]
    fn apply_preemphasis_correctness() {
        let input = vec![1.0f32, 2.0, 3.0, 4.0];
        let result = apply_preemphasis(&input, 0.97);
        assert_eq!(result.len(), 4);
        assert!((result[0] - 1.0).abs() < 1e-6); // first sample unchanged
        assert!((result[1] - (2.0 - 0.97 * 1.0)).abs() < 1e-5);
        assert!((result[2] - (3.0 - 0.97 * 2.0)).abs() < 1e-5);
        assert!((result[3] - (4.0 - 0.97 * 3.0)).abs() < 1e-5);
    }

    #[test]
    fn apply_preemphasis_empty() {
        let result = apply_preemphasis(&[], 0.97);
        assert!(result.is_empty());
    }

    #[test]
    fn mel_filterbank_shape() {
        let filters = build_mel_filterbank(N_FFT, N_MELS, SAMPLE_RATE, MelScale::HTK, false);
        assert_eq!(filters.len(), N_MELS);
        let n_freqs = N_FFT / 2 + 1;
        for f in &filters {
            assert_eq!(f.len(), n_freqs);
        }
    }

    #[test]
    fn mel_filterbank_non_negative() {
        for scale in [MelScale::HTK, MelScale::Slaney] {
            let filters = build_mel_filterbank(N_FFT, N_MELS, SAMPLE_RATE, scale, false);
            for (m, f) in filters.iter().enumerate() {
                for (k, &v) in f.iter().enumerate() {
                    assert!(v >= 0.0, "Negative filter value at mel={}, freq_bin={}: {}", m, k, v);
                }
            }
        }
    }

    #[test]
    fn mel_filterbank_triangular_shape() {
        // Most filters should have a triangular shape with peak <= 1.0.
        // Skip filter 0 which may be all zeros (starts at 0 Hz, very narrow).
        let filters = build_mel_filterbank(N_FFT, N_MELS, SAMPLE_RATE, MelScale::HTK, false);
        let mut nonzero_count = 0;
        for (_m, f) in filters.iter().enumerate().skip(1) {
            let max_val = f.iter().cloned().fold(0.0f32, f32::max);
            if max_val > 0.0 {
                nonzero_count += 1;
                assert!(max_val <= 1.01, "Filter {} peak {} > 1.0", _m, max_val);
            }
        }
        // Most filters should be non-zero
        assert!(nonzero_count > N_MELS / 2, "Too few non-zero filters: {}", nonzero_count);
    }

    #[test]
    fn mel_filterbank_slaney_norm_changes_values() {
        let unnormed = build_mel_filterbank(N_FFT, N_MELS, SAMPLE_RATE, MelScale::Slaney, false);
        let normed = build_mel_filterbank(N_FFT, N_MELS, SAMPLE_RATE, MelScale::Slaney, true);

        // Slaney normalization should change the filter values
        let mut diff_sum = 0.0f64;
        for m in 0..N_MELS {
            for k in 0..unnormed[m].len() {
                diff_sum += (normed[m][k] - unnormed[m][k]).abs() as f64;
            }
        }
        assert!(diff_sum > 0.0, "Slaney normalization should change filter values");
    }

    #[test]
    fn canary_vs_parakeet_different_output() {
        // Same audio should produce different features with different configs
        let audio: Vec<f32> = (0..8000)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 16000.0).sin() * 0.3)
            .collect();
        let (canary_feat, canary_frames) = extract_features_with_config(&audio, &CANARY_CONFIG);
        let (parakeet_feat, parakeet_frames) = extract_features_with_config(&audio, &PARAKEET_CONFIG);

        assert_eq!(canary_frames, parakeet_frames);
        // Features should differ due to different mel scale, pre-emphasis, etc.
        let diff: f32 = canary_feat.iter().zip(parakeet_feat.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 0.0, "Canary and Parakeet features should differ");
    }
}
