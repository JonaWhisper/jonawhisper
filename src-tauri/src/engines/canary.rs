use super::*;

pub struct CanaryEngine;

fn storage_dir() -> String {
    crate::state::models_dir().join("canary").to_string_lossy().to_string()
}

impl ASREngine for CanaryEngine {
    fn engine_id(&self) -> &str { "canary" }
    fn display_name(&self) -> &str { "Canary" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "canary:180m-flash-int8".into(),
                engine_id: "canary".into(),
                label: "Canary 180M Flash INT8".into(),
                filename: "180m-flash-int8".into(), // directory name
                url: String::new(), // not used for MultiFile
                size: 213_284_662, // total: encoder + decoder + vocab
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/canary-180m-flash-onnx/resolve/main/encoder-model.int8.onnx".into(),
                            size: 133_710_896,
                        },
                        DownloadFile {
                            filename: "decoder-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/canary-180m-flash-onnx/resolve/main/decoder-model.int8.onnx".into(),
                            size: 79_520_211,
                        },
                        DownloadFile {
                            filename: "vocab.txt".into(),
                            url: "https://huggingface.co/istupakov/canary-180m-flash-onnx/resolve/main/vocab.txt".into(),
                            size: 53_555,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                wer: Some(1.87),
                rtf: Some(0.15),
                recommended: false,
                params: Some(0.182),
                ram: Some(300_000_000),
                lang_codes: Some(vec!["fr".into(), "en".into(), "de".into(), "es".into()]),
                runtime: Some("ort".into()),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "fr".into(), label: "Français".into() },
            Language { code: "en".into(), label: "English".into() },
            Language { code: "de".into(), label: "Deutsch".into() },
            Language { code: "es".into(), label: "Español".into() },
        ]
    }

    fn description(&self) -> &str {
        "NVIDIA Canary encoder-decoder ASR. Ultra-light (182M params), beats Whisper Medium quality."
    }
}
