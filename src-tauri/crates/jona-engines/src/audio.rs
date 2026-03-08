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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write_mono_wav(path: &Path, samples: &[f32], sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for &s in samples {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn write_stereo_wav(path: &Path, left: &[f32], right: &[f32], sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..left.len() {
            writer.write_sample(left[i]).unwrap();
            writer.write_sample(right[i]).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn write_i16_mono_wav(path: &Path, samples: &[i16], sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for &s in samples {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn read_mono_f32_wav() {
        let dir = std::env::temp_dir().join("jona_audio_test_mono");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("mono.wav");
        let samples = vec![0.0f32, 0.5, -0.5, 1.0, -1.0];
        write_mono_wav(&path, &samples, 16000);

        let result = read_wav_f32(&path).unwrap();
        assert_eq!(result.len(), 5);
        assert!((result[0] - 0.0).abs() < 1e-5);
        assert!((result[1] - 0.5).abs() < 1e-5);
        assert!((result[2] - (-0.5)).abs() < 1e-5);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_stereo_wav_averages_channels() {
        let dir = std::env::temp_dir().join("jona_audio_test_stereo");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("stereo.wav");

        let left =  vec![1.0f32, 0.0, 0.5];
        let right = vec![0.0f32, 1.0, 0.5];
        write_stereo_wav(&path, &left, &right, 16000);

        let result = read_wav_f32(&path).unwrap();
        assert_eq!(result.len(), 3);
        assert!((result[0] - 0.5).abs() < 1e-5); // (1.0+0.0)/2
        assert!((result[1] - 0.5).abs() < 1e-5); // (0.0+1.0)/2
        assert!((result[2] - 0.5).abs() < 1e-5); // (0.5+0.5)/2

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_i16_wav_normalizes() {
        let dir = std::env::temp_dir().join("jona_audio_test_i16");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("i16.wav");
        // i16 max is 32767, so 16384 should be ~0.5
        write_i16_mono_wav(&path, &[0, 16384, -16384], 16000);

        let result = read_wav_f32(&path).unwrap();
        assert_eq!(result.len(), 3);
        assert!((result[0]).abs() < 1e-4);
        assert!((result[1] - 0.5).abs() < 0.01);
        assert!((result[2] - (-0.5)).abs() < 0.01);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_nonexistent_file_returns_error() {
        let path = PathBuf::from("/tmp/jona_audio_test_nonexistent_file.wav");
        let result = read_wav_f32(&path);
        assert!(result.is_err());
    }

    #[test]
    fn read_empty_mono_wav() {
        let dir = std::env::temp_dir().join("jona_audio_test_empty");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("empty.wav");
        write_mono_wav(&path, &[], 16000);

        let result = read_wav_f32(&path).unwrap();
        assert!(result.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
