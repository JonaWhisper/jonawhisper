use super::*;
use crate::process_runner;
use std::path::Path;

pub struct VoskEngine;

impl ASREngine for VoskEngine {
    fn engine_id(&self) -> &str { "vosk" }
    fn display_name(&self) -> &str { "Vosk" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "vosk:en-small".into(), engine_id: "vosk".into(),
                label: "English Small".into(), filename: "vosk-model-small-en-us-0.15".into(),
                url: "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip".into(),
                size: "40 Mo".into(), storage_dir: "~/.cache/vosk".into(),
                download_type: DownloadType::ZipArchive, download_marker: Some("conf/model.conf".into()),
            },
            ASRModel {
                id: "vosk:en-large".into(), engine_id: "vosk".into(),
                label: "English Large".into(), filename: "vosk-model-en-us-0.22".into(),
                url: "https://alphacephei.com/vosk/models/vosk-model-en-us-0.22.zip".into(),
                size: "1.8 Go".into(), storage_dir: "~/.cache/vosk".into(),
                download_type: DownloadType::ZipArchive, download_marker: Some("conf/model.conf".into()),
            },
            ASRModel {
                id: "vosk:fr-small".into(), engine_id: "vosk".into(),
                label: "Français Small".into(), filename: "vosk-model-small-fr-0.22".into(),
                url: "https://alphacephei.com/vosk/models/vosk-model-small-fr-0.22.zip".into(),
                size: "41 Mo".into(), storage_dir: "~/.cache/vosk".into(),
                download_type: DownloadType::ZipArchive, download_marker: Some("conf/model.conf".into()),
            },
            ASRModel {
                id: "vosk:fr-large".into(), engine_id: "vosk".into(),
                label: "Français Large".into(), filename: "vosk-model-fr-0.22".into(),
                url: "https://alphacephei.com/vosk/models/vosk-model-fr-0.22.zip".into(),
                size: "1.4 Go".into(), storage_dir: "~/.cache/vosk".into(),
                download_type: DownloadType::ZipArchive, download_marker: Some("conf/model.conf".into()),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "en".into(), label: "English".into() },
            Language { code: "fr".into(), label: "Français".into() },
        ]
    }

    fn install_hint(&self) -> &str { "pip install vosk" }

    fn resolve_executable(&self) -> Option<String> {
        find_executable("vosk-transcriber", &[])
    }

    fn transcribe(&self, model: &ASRModel, audio_path: &Path, _language: &str) -> Result<String, EngineError> {
        let exe = self.resolve_executable()
            .ok_or_else(|| EngineError::EngineUnavailable {
                engine_id: self.engine_id().into(),
                install_hint: self.install_hint().into(),
            })?;

        let model_path = model.local_path();
        if !model_path.exists() {
            return Err(EngineError::ModelNotFound(model_path.display().to_string()));
        }

        let args = vec![
            "-m".to_string(), model_path.to_string_lossy().to_string(),
            "-i".to_string(), audio_path.to_string_lossy().to_string(),
            "-o".to_string(), "-".to_string(),
        ];

        let result = process_runner::run(&exe, &args)?;
        Ok(result.stdout)
    }
}
