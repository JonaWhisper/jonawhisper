use super::*;
use crate::process_runner;
use std::path::Path;

pub struct MLXWhisperEngine;

impl ASREngine for MLXWhisperEngine {
    fn engine_id(&self) -> &str { "mlx-whisper" }
    fn display_name(&self) -> &str { "MLX Whisper" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "mlx-whisper:tiny".into(), engine_id: "mlx-whisper".into(),
                label: "Tiny".into(), filename: "models--mlx-community--whisper-tiny-mlx".into(),
                url: "mlx-community/whisper-tiny-mlx".into(),
                size: "75 Mo".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "mlx-whisper:base".into(), engine_id: "mlx-whisper".into(),
                label: "Base".into(), filename: "models--mlx-community--whisper-base-mlx".into(),
                url: "mlx-community/whisper-base-mlx".into(),
                size: "142 Mo".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "mlx-whisper:small".into(), engine_id: "mlx-whisper".into(),
                label: "Small".into(), filename: "models--mlx-community--whisper-small-mlx".into(),
                url: "mlx-community/whisper-small-mlx".into(),
                size: "466 Mo".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "mlx-whisper:medium".into(), engine_id: "mlx-whisper".into(),
                label: "Medium".into(), filename: "models--mlx-community--whisper-medium-mlx".into(),
                url: "mlx-community/whisper-medium-mlx".into(),
                size: "1.5 Go".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "mlx-whisper:large-v3-turbo".into(), engine_id: "mlx-whisper".into(),
                label: "Large V3 Turbo".into(), filename: "models--mlx-community--whisper-large-v3-turbo".into(),
                url: "mlx-community/whisper-large-v3-turbo".into(),
                size: "1.6 Go".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "mlx-whisper:large-v3".into(), engine_id: "mlx-whisper".into(),
                label: "Large V3".into(), filename: "models--mlx-community--whisper-large-v3-mlx".into(),
                url: "mlx-community/whisper-large-v3-mlx".into(),
                size: "3.1 Go".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "mlx-whisper:large-v3-turbo-q4".into(), engine_id: "mlx-whisper".into(),
                label: "Large V3 Turbo Q4".into(), filename: "models--mlx-community--whisper-large-v3-turbo-q4".into(),
                url: "mlx-community/whisper-large-v3-turbo-q4".into(),
                size: "534 Mo".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> { common_languages() }

    fn install_hint(&self) -> &str { "pip install mlx-whisper" }

    fn resolve_executable(&self) -> Option<String> {
        find_executable("mlx_whisper", &[])
    }

    fn transcribe(&self, model: &ASRModel, audio_path: &Path, language: &str) -> Result<String, EngineError> {
        let exe = self.resolve_executable()
            .ok_or_else(|| EngineError::EngineUnavailable {
                engine_id: self.engine_id().into(),
                install_hint: self.install_hint().into(),
            })?;

        let mut args = vec![
            audio_path.to_string_lossy().to_string(),
            "--model".to_string(), model.url.clone(),
            "--output_format".to_string(), "txt".to_string(),
            "--output_dir".to_string(), "/tmp".to_string(),
        ];

        if language != "auto" {
            args.push("--language".to_string());
            args.push(language.to_string());
        }

        let result = process_runner::run(&exe, &args)?;

        // mlx_whisper writes to a .txt file in output_dir
        let stem = audio_path.file_stem().unwrap_or_default().to_string_lossy();
        let txt_path = format!("/tmp/{}.txt", stem);
        Ok(std::fs::read_to_string(&txt_path)
            .map(|s| s.trim().to_string())
            .unwrap_or(result.stdout))
    }
}
