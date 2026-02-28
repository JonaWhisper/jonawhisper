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
                size: 26_000_000, storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
                wer: Some(8.0), rtf: Some(0.01),
            },
            ASRModel {
                id: "moonshine:base".into(), engine_id: "moonshine".into(),
                label: "Base".into(), filename: "models--UsefulSensors--moonshine-base".into(),
                url: "UsefulSensors/moonshine-base".into(),
                size: 61_000_000, storage_dir: "~/.cache/huggingface/hub".into(),
                download_type: DownloadType::HuggingFaceRepo, download_marker: Some("refs/main".into()),
                wer: Some(5.0), rtf: Some(0.03),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "en".into(), label: "English".into() },
        ]
    }

    fn description(&self) -> &str { "Ultra-fast, tiny models. English only." }
    fn install_hint(&self) -> &str { "pip install useful-moonshine" }

    fn resolve_executable(&self) -> Option<String> {
        // Moonshine uses Python module
        find_executable("python3", &[])
    }

    fn recommended_model_id(&self, _language: &str) -> Option<String> {
        Some("moonshine:base".into())
    }

    fn transcribe(&self, model: &ASRModel, audio_path: &Path, _language: &str) -> Result<String, EngineError> {
        let python = self.resolve_executable()
            .ok_or_else(|| EngineError::EngineUnavailable {
                engine_id: self.engine_id().into(),
                install_hint: self.install_hint().into(),
            })?;

        // Pass audio path and model name via sys.argv to avoid shell injection
        let model_name = if model.id.contains("tiny") { "moonshine/tiny" } else { "moonshine/base" };
        let script = "import sys, moonshine; print(moonshine.transcribe(sys.argv[1], model=sys.argv[2])[0])";

        let args = vec![
            "-c".to_string(),
            script.to_string(),
            audio_path.to_string_lossy().to_string(),
            model_name.to_string(),
        ];
        let result = process_runner::run(&python, &args)?;
        Ok(result.stdout)
    }
}
