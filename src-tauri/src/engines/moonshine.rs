use super::*;
use crate::process_runner;
use std::path::Path;

pub struct MoonshineEngine;

impl ASREngine for MoonshineEngine {
    fn engine_id(&self) -> &str { "moonshine" }
    fn display_name(&self) -> &str { "Moonshine" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "moonshine:tiny".into(), engine_id: "moonshine".into(),
                label: "Tiny".into(), filename: "models--UsefulSensors--moonshine-tiny".into(),
                url: "UsefulSensors/moonshine-tiny".into(),
                size: "26 Mo".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
            ASRModel {
                id: "moonshine:base".into(), engine_id: "moonshine".into(),
                label: "Base".into(), filename: "models--UsefulSensors--moonshine-base".into(),
                url: "UsefulSensors/moonshine-base".into(),
                size: "61 Mo".into(), storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "en".into(), label: "English".into() },
        ]
    }

    fn install_hint(&self) -> &str { "pip install useful-moonshine" }

    fn resolve_executable(&self) -> Option<String> {
        // Moonshine uses Python module
        find_executable("python3", &[])
    }

    fn transcribe(&self, model: &ASRModel, audio_path: &Path, _language: &str) -> Result<String, EngineError> {
        let python = self.resolve_executable()
            .ok_or_else(|| EngineError::EngineUnavailable {
                engine_id: self.engine_id().into(),
                install_hint: self.install_hint().into(),
            })?;

        // Use a Python one-liner to run moonshine
        let model_name = if model.id.contains("tiny") { "moonshine/tiny" } else { "moonshine/base" };
        let script = format!(
            "import moonshine; print(moonshine.transcribe('{}', model='{}')[0])",
            audio_path.to_string_lossy(),
            model_name
        );

        let args = vec!["-c".to_string(), script];
        let result = process_runner::run(&python, &args)?;
        Ok(result.stdout)
    }
}
