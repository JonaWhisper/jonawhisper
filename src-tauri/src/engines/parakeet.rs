use super::*;

pub struct ParakeetEngine;

fn storage_dir() -> String {
    crate::state::models_dir().join("parakeet").to_string_lossy().to_string()
}

impl ASREngine for ParakeetEngine {
    fn engine_id(&self) -> &str { "parakeet" }
    fn display_name(&self) -> &str { "Parakeet" }

    fn models(&self) -> Vec<ASRModel> {
        vec![
            ASRModel {
                id: "parakeet:tdt-0.6b-v3-int8".into(),
                engine_id: "parakeet".into(),
                label: "Parakeet TDT 0.6B v3 INT8".into(),
                filename: "tdt-0.6b-v3-int8".into(),
                url: String::new(),
                size: 683_574_784 + 19_078_554 + 96_153, // encoder + decoder + vocab
                storage_dir: storage_dir(),
                download_type: DownloadType::MultiFile {
                    files: vec![
                        DownloadFile {
                            filename: "encoder-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/encoder-model.int8.onnx".into(),
                            size: 683_574_784,
                        },
                        DownloadFile {
                            filename: "decoder_joint-model.int8.onnx".into(),
                            url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/decoder_joint-model.int8.onnx".into(),
                            size: 19_078_554,
                        },
                        DownloadFile {
                            filename: "vocab.txt".into(),
                            url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/vocab.txt".into(),
                            size: 96_153,
                        },
                    ],
                },
                download_marker: Some(".complete".into()),
                wer: Some(1.5),
                rtf: Some(0.10),
                recommended: false,
                params: Some(0.6),
                ram: Some(750_000_000),
                lang_codes: Some(vec![
                    "en".into(), "fr".into(), "de".into(), "es".into(), "it".into(),
                    "pt".into(), "nl".into(), "pl".into(), "ru".into(), "uk".into(),
                    "sv".into(), "da".into(), "fi".into(), "ro".into(), "hu".into(),
                    "cs".into(), "sk".into(), "bg".into(), "hr".into(), "sl".into(),
                    "el".into(), "lt".into(), "lv".into(), "et".into(), "mt".into(),
                ]),
                runtime: Some("ort".into()),
            },
        ]
    }

    fn supported_languages(&self) -> Vec<Language> {
        vec![
            Language { code: "en".into(), label: "English".into() },
            Language { code: "fr".into(), label: "Français".into() },
            Language { code: "de".into(), label: "Deutsch".into() },
            Language { code: "es".into(), label: "Español".into() },
            Language { code: "it".into(), label: "Italiano".into() },
            Language { code: "pt".into(), label: "Português".into() },
            Language { code: "nl".into(), label: "Nederlands".into() },
            Language { code: "pl".into(), label: "Polski".into() },
            Language { code: "ru".into(), label: "Русский".into() },
            Language { code: "uk".into(), label: "Українська".into() },
            Language { code: "sv".into(), label: "Svenska".into() },
            Language { code: "da".into(), label: "Dansk".into() },
            Language { code: "fi".into(), label: "Suomi".into() },
            Language { code: "ro".into(), label: "Română".into() },
            Language { code: "hu".into(), label: "Magyar".into() },
            Language { code: "cs".into(), label: "Čeština".into() },
            Language { code: "sk".into(), label: "Slovenčina".into() },
            Language { code: "bg".into(), label: "Български".into() },
            Language { code: "hr".into(), label: "Hrvatski".into() },
            Language { code: "sl".into(), label: "Slovenščina".into() },
            Language { code: "el".into(), label: "Ελληνικά".into() },
            Language { code: "lt".into(), label: "Lietuvių".into() },
            Language { code: "lv".into(), label: "Latviešu".into() },
            Language { code: "et".into(), label: "Eesti".into() },
            Language { code: "mt".into(), label: "Malti".into() },
        ]
    }

    fn description(&self) -> &str {
        "NVIDIA Parakeet TDT transducer ASR. 25 European languages with auto-detection, excellent quality."
    }
}
