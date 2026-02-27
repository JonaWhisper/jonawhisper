use super::*;
use crate::process_runner;
use std::path::Path;

pub struct WhisperEngine;

impl ASREngine for WhisperEngine {
    fn engine_id(&self) -> &str { "whisper" }
    fn display_name(&self) -> &str { "Whisper" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "whisper:tiny".into(), engine_id: "whisper".into(),
                label: "Tiny".into(), filename: "ggml-tiny.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".into(),
                size: "75 Mo".into(), storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
            },
            ASRModel {
                id: "whisper:base".into(), engine_id: "whisper".into(),
                label: "Base".into(), filename: "ggml-base.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
                size: "142 Mo".into(), storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
            },
            ASRModel {
                id: "whisper:small".into(), engine_id: "whisper".into(),
                label: "Small".into(), filename: "ggml-small.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
                size: "466 Mo".into(), storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
            },
            ASRModel {
                id: "whisper:medium".into(), engine_id: "whisper".into(),
                label: "Medium".into(), filename: "ggml-medium.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".into(),
                size: "1.5 Go".into(), storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
            },
            ASRModel {
                id: "whisper:large-v3-turbo".into(), engine_id: "whisper".into(),
                label: "Large V3 Turbo".into(), filename: "ggml-large-v3-turbo.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin".into(),
                size: "1.6 Go".into(), storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
            },
            ASRModel {
                id: "whisper:large-v3".into(), engine_id: "whisper".into(),
                label: "Large V3".into(), filename: "ggml-large-v3.bin".into(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin".into(),
                size: "3.1 Go".into(), storage_dir: "~/.local/share/whisper-cpp".into(),
                download_type: DownloadType::SingleFile, download_marker: None,
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> { common_languages() }

    fn install_hint(&self) -> &str { "brew install whisper-cpp" }

    fn resolve_executable(&self) -> Option<String> {
        find_executable("whisper-cli", &[])
            .or_else(|| find_executable("whisper-cpp", &[]))
            .or_else(|| find_executable("main", &["/opt/homebrew/bin"]))
    }

    fn transcribe(&self, model: &ASRModel, audio_path: &Path, language: &str) -> Result<String, EngineError> {
        let exe = self.resolve_executable()
            .ok_or_else(|| EngineError::EngineUnavailable {
                engine_id: self.engine_id().into(),
                install_hint: self.install_hint().into(),
            })?;

        let model_path = model.local_path();
        if !model_path.exists() {
            return Err(EngineError::ModelNotFound(model_path.display().to_string()));
        }

        let mut args = vec![
            "--model".to_string(), model_path.to_string_lossy().to_string(),
            "--file".to_string(), audio_path.to_string_lossy().to_string(),
            "--output-txt".to_string(),
            "--no-timestamps".to_string(),
        ];

        if language != "auto" {
            args.push("--language".to_string());
            args.push(language.to_string());
        }

        let result = process_runner::run(&exe, &args)?;
        Ok(result.stdout)
    }
}
