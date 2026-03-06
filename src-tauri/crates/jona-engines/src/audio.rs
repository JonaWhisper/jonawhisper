//! Shared audio utilities for engine crates.

use super::EngineError;
use std::path::Path;

/// Read a WAV file and convert to f32 mono samples.
pub fn read_wav_f32(path: &Path) -> Result<Vec<f32>, EngineError> {
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
