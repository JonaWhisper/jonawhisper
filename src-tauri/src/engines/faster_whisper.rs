use super::*;
use crate::process_runner;
use std::path::Path;

pub struct FasterWhisperEngine;

impl ASREngine for FasterWhisperEngine {
    fn engine_id(&self) -> &str { "faster-whisper" }
    fn display_name(&self) -> &str { "Faster Whisper" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "faster-whisper:tiny".into(), engine_id: "faster-whisper".into(),
                label: "Tiny".into(), filename: "models--Systran--faster-whisper-tiny".into(),
                url: "Systran/faster-whisper-tiny".into(),
                size: "75 Mo".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "faster-whisper:base".into(), engine_id: "faster-whisper".into(),
                label: "Base".into(), filename: "models--Systran--faster-whisper-base".into(),
                url: "Systran/faster-whisper-base".into(),
                size: "142 Mo".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "faster-whisper:small".into(), engine_id: "faster-whisper".into(),
                label: "Small".into(), filename: "models--Systran--faster-whisper-small".into(),
                url: "Systran/faster-whisper-small".into(),
                size: "466 Mo".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "faster-whisper:medium".into(), engine_id: "faster-whisper".into(),
                label: "Medium".into(), filename: "models--Systran--faster-whisper-medium".into(),
                url: "Systran/faster-whisper-medium".into(),
                size: "1.5 Go".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "faster-whisper:large-v3".into(), engine_id: "faster-whisper".into(),
                label: "Large V3".into(), filename: "models--Systran--faster-whisper-large-v3".into(),
                url: "Systran/faster-whisper-large-v3".into(),
                size: "3.1 Go".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "faster-whisper:large-v3-turbo".into(), engine_id: "faster-whisper".into(),
                label: "Large V3 Turbo".into(), filename: "models--Systran--faster-whisper-large-v3-turbo".into(),
                url: "Systran/faster-whisper-large-v3-turbo".into(),
                size: "1.6 Go".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "faster-whisper:distil-large-v3".into(), engine_id: "faster-whisper".into(),
                label: "Distil Large V3".into(), filename: "models--Systran--faster-distil-whisper-large-v3".into(),
                url: "Systran/faster-distil-whisper-large-v3".into(),
                size: "1.5 Go".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> { common_languages() }

    fn install_hint(&self) -> &str { "pip install whisper-ctranslate2" }

    fn resolve_executable(&self) -> Option<String> {
        find_executable("whisper-ctranslate2", &[])
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
        } else {
            args.push("--language".to_string());
            args.push("auto".to_string());
        }

        let result = process_runner::run(&exe, &args)?;

        let stem = audio_path.file_stem().unwrap_or_default().to_string_lossy();
        let txt_path = format!("/tmp/{}.txt", stem);
        Ok(std::fs::read_to_string(&txt_path)
            .map(|s| s.trim().to_string())
            .unwrap_or(result.stdout))
    }
}
