use jona_types::{
    CloudProvider, FieldType, PresetField, Provider, ProviderError, ProviderPreset,
    ProviderRegistration, TranscriptionResult,
};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

// ── Shared helpers ──────────────────────────────────────────────────────────

/// AWS regions that support Transcribe.
const AWS_REGIONS: &[(&str, &str)] = &[
    ("us-east-1", "US East (N. Virginia)"),
    ("us-east-2", "US East (Ohio)"),
    ("us-west-1", "US West (N. California)"),
    ("us-west-2", "US West (Oregon)"),
    ("eu-west-1", "Europe (Ireland)"),
    ("eu-west-2", "Europe (London)"),
    ("eu-central-1", "Europe (Frankfurt)"),
    ("ap-southeast-1", "Asia Pacific (Singapore)"),
    ("ap-southeast-2", "Asia Pacific (Sydney)"),
    ("ap-northeast-1", "Asia Pacific (Tokyo)"),
    ("ap-northeast-2", "Asia Pacific (Seoul)"),
    ("ap-south-1", "Asia Pacific (Mumbai)"),
    ("ca-central-1", "Canada (Central)"),
    ("sa-east-1", "South America (São Paulo)"),
    ("me-south-1", "Middle East (Bahrain)"),
    ("af-south-1", "Africa (Cape Town)"),
];

/// Build an AWS SDK config from provider extra fields.
fn aws_config(provider: &Provider) -> Result<aws_config::SdkConfig, ProviderError> {
    let access_key = provider
        .extra
        .get("access_key")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ProviderError::NotConfigured("AWS Access Key is not configured".into()))?;

    let secret_key = provider
        .extra
        .get("secret_key")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ProviderError::NotConfigured("AWS Secret Key is not configured".into()))?;

    let region = provider
        .extra
        .get("region")
        .map(|s| s.as_str())
        .unwrap_or("us-east-1");

    if !AWS_REGIONS.iter().any(|(id, _)| *id == region) {
        return Err(ProviderError::NotConfigured(
            "Invalid or missing AWS region".into(),
        ));
    }

    let creds = aws_credential_types::Credentials::new(
        access_key, secret_key, None, None, "jona-whisper",
    );

    let config = aws_config::SdkConfig::builder()
        .region(aws_config::Region::new(region.to_string()))
        .credentials_provider(aws_credential_types::provider::SharedCredentialsProvider::new(creds))
        .build();

    Ok(config)
}

/// Convert a 2-letter language code to an AWS Transcribe language code (BCP-47).
fn aws_language_code(lang: &str) -> String {
    if lang == "auto" {
        return "en-US".to_string();
    }
    if lang.contains('-') || lang.contains('_') {
        return lang.replace('_', "-");
    }
    match lang {
        "en" => "en-US",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-US",
        "it" => "it-IT",
        "pt" => "pt-BR",
        "nl" => "nl-NL",
        "pl" => "pl-PL",
        "ru" => "ru-RU",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "zh" => "zh-CN",
        "ar" => "ar-SA",
        "hi" => "hi-IN",
        "sv" => "sv-SE",
        "da" => "da-DK",
        "fi" => "fi-FI",
        "nb" => "nb-NO",
        "tr" => "tr-TR",
        "uk" => "uk-UA",
        "cs" => "cs-CZ",
        "el" => "el-GR",
        "ro" => "ro-RO",
        "hu" => "hu-HU",
        "th" => "th-TH",
        "vi" => "vi-VN",
        "id" => "id-ID",
        "ms" => "ms-MY",
        code => return format!("{}-{}", code, code.to_uppercase()),
    }
    .to_string()
}

// ── Streaming backend ───────────────────────────────────────────────────────

/// AWS Transcribe Streaming — sends audio directly, no S3 needed.
pub struct AwsTranscribeStreamingBackend;

impl CloudProvider for AwsTranscribeStreamingBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        _model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        let config = aws_config(provider)?;
        let audio_bytes = std::fs::read(audio_path)?;

        // Extract sample rate from WAV header (bytes 24-27, little-endian u32)
        let sample_rate = if audio_bytes.len() >= 28 {
            u32::from_le_bytes([audio_bytes[24], audio_bytes[25], audio_bytes[26], audio_bytes[27]])
        } else {
            16000
        };

        // Strip WAV header (first 44 bytes) to get raw PCM
        let pcm_data = if audio_bytes.len() > 44 {
            &audio_bytes[44..]
        } else {
            &audio_bytes
        };

        let lang_code = aws_language_code(language);

        // Run the async streaming transcription on the current tokio runtime
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                streaming_transcribe(&config, pcm_data, sample_rate as i32, &lang_code).await
            })
        })?;

        Ok(TranscriptionResult::text_only(result))
    }

    fn chat_completion<'a>(
        &'a self,
        provider: &'a Provider,
        _model: &'a str,
        _system: &'a str,
        _user_message: &'a str,
        _temperature: f32,
        _max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            Err(ProviderError::NotConfigured(format!(
                "Provider '{}' does not support LLM chat",
                provider.name
            )))
        })
    }

    fn list_models<'a>(
        &'a self,
        _provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move { Ok(vec!["default".into()]) })
    }
}

/// Perform streaming transcription via AWS Transcribe Streaming SDK.
async fn streaming_transcribe(
    config: &aws_config::SdkConfig,
    pcm_data: &[u8],
    sample_rate: i32,
    language_code: &str,
) -> Result<String, ProviderError> {
    use aws_sdk_transcribestreaming::types::{
        AudioEvent, AudioStream, LanguageCode, MediaEncoding, TranscriptResultStream,
    };
    use aws_sdk_transcribestreaming::primitives::Blob;
    use aws_smithy_http::event_stream::EventStreamSender;

    let client = aws_sdk_transcribestreaming::Client::new(config);

    let lang = language_code
        .parse::<LanguageCode>()
        .unwrap_or(LanguageCode::EnUs);

    // Build the audio stream from PCM chunks
    let pcm_owned = pcm_data.to_vec();
    let audio_stream = futures_util::stream::iter(
        pcm_owned
            .chunks(8192)
            .map(|chunk| {
                Ok(AudioStream::AudioEvent(
                    AudioEvent::builder()
                        .audio_chunk(Blob::new(chunk.to_vec()))
                        .build(),
                ))
            })
            .collect::<Vec<_>>(),
    );

    let sender = EventStreamSender::from(audio_stream);

    let mut output = client
        .start_stream_transcription()
        .language_code(lang)
        .media_encoding(MediaEncoding::Pcm)
        .media_sample_rate_hertz(sample_rate)
        .audio_stream(sender)
        .send()
        .await
        .map_err(|e| ProviderError::Http(format!("AWS Transcribe streaming error: {e}")))?;

    // Collect final transcript results
    let mut transcript = String::new();
    while let Some(event) = output
        .transcript_result_stream
        .recv()
        .await
        .map_err(|e| ProviderError::Http(format!("AWS Transcribe stream recv error: {e}")))?
    {
        if let TranscriptResultStream::TranscriptEvent(te) = event {
            if let Some(t) = te.transcript() {
                for result in t.results() {
                    if !result.is_partial() {
                        for alt in result.alternatives() {
                            if let Some(text) = alt.transcript() {
                                if !transcript.is_empty() {
                                    transcript.push(' ');
                                }
                                transcript.push_str(text);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(transcript)
}

// ── Batch backend ───────────────────────────────────────────────────────────

/// AWS Transcribe Batch — upload to S3, start job, poll until complete.
pub struct AwsTranscribeBatchBackend;

impl CloudProvider for AwsTranscribeBatchBackend {
    fn transcribe(
        &self,
        provider: &Provider,
        _model: &str,
        audio_path: &Path,
        language: &str,
    ) -> Result<TranscriptionResult, ProviderError> {
        let config = aws_config(provider)?;
        let audio_bytes = std::fs::read(audio_path)?;

        let s3_bucket = provider
            .extra
            .get("s3_bucket")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ProviderError::NotConfigured("S3 bucket is not configured".into()))?
            .clone();

        let lang_code = aws_language_code(language);

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                batch_transcribe(&config, &audio_bytes, &s3_bucket, &lang_code).await
            })
        })?;

        Ok(TranscriptionResult::text_only(result))
    }

    fn chat_completion<'a>(
        &'a self,
        provider: &'a Provider,
        _model: &'a str,
        _system: &'a str,
        _user_message: &'a str,
        _temperature: f32,
        _max_tokens: u32,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            Err(ProviderError::NotConfigured(format!(
                "Provider '{}' does not support LLM chat",
                provider.name
            )))
        })
    }

    fn list_models<'a>(
        &'a self,
        _provider: &'a Provider,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, ProviderError>> + Send + 'a>> {
        Box::pin(async move { Ok(vec!["default".into()]) })
    }
}

/// Perform batch transcription: upload to S3 → start job → poll → download result → cleanup.
async fn batch_transcribe(
    config: &aws_config::SdkConfig,
    audio_bytes: &[u8],
    s3_bucket: &str,
    language_code: &str,
) -> Result<String, ProviderError> {
    use aws_sdk_s3::primitives::ByteStream;
    use aws_sdk_transcribe::types::{LanguageCode, Media, TranscriptionJobStatus};

    let s3_client = aws_sdk_s3::Client::new(config);
    let transcribe_client = aws_sdk_transcribe::Client::new(config);

    let job_id = uuid::Uuid::new_v4().to_string();
    let s3_key = format!("jona-whisper/tmp/{}.wav", job_id);
    let s3_uri = format!("s3://{}/{}", s3_bucket, s3_key);

    // 1. Upload audio to S3
    s3_client
        .put_object()
        .bucket(s3_bucket)
        .key(&s3_key)
        .content_type("audio/wav")
        .body(ByteStream::from(audio_bytes.to_vec()))
        .send()
        .await
        .map_err(|e| ProviderError::Http(format!("S3 upload failed: {e}")))?;

    // 2. Start transcription job
    let lang = language_code
        .parse::<LanguageCode>()
        .unwrap_or(LanguageCode::EnUs);

    let job_name = format!("jona-{}", job_id);
    transcribe_client
        .start_transcription_job()
        .transcription_job_name(&job_name)
        .language_code(lang)
        .media_format(aws_sdk_transcribe::types::MediaFormat::Wav)
        .media(Media::builder().media_file_uri(&s3_uri).build())
        .send()
        .await
        .map_err(|e| ProviderError::Http(format!("StartTranscriptionJob failed: {e}")))?;

    // 3. Poll until complete (max ~120s)
    let mut transcript_uri = String::new();
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let resp = transcribe_client
            .get_transcription_job()
            .transcription_job_name(&job_name)
            .send()
            .await
            .map_err(|e| ProviderError::Http(format!("GetTranscriptionJob failed: {e}")))?;

        if let Some(job) = resp.transcription_job() {
            match job.transcription_job_status() {
                Some(TranscriptionJobStatus::Completed) => {
                    if let Some(t) = job.transcript() {
                        transcript_uri = t.transcript_file_uri().unwrap_or_default().to_string();
                    }
                    break;
                }
                Some(TranscriptionJobStatus::Failed) => {
                    let reason = job.failure_reason().unwrap_or("Unknown error");
                    // Cleanup S3 before returning error
                    let _ = s3_client
                        .delete_object()
                        .bucket(s3_bucket)
                        .key(&s3_key)
                        .send()
                        .await;
                    return Err(ProviderError::Api {
                        status: 500,
                        body: format!("Transcription failed: {reason}"),
                    });
                }
                _ => continue,
            }
        }
    }

    // 4. Download transcript JSON from the result URI
    let text = if !transcript_uri.is_empty() {
        let resp = reqwest::get(&transcript_uri)
            .await
            .map_err(|e| ProviderError::Http(format!("Failed to download transcript: {e}")))?;
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;
        json.pointer("/results/transcripts/0/transcript")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        return Err(ProviderError::Http(
            "Transcription job timed out".to_string(),
        ));
    };

    // 5. Cleanup: delete temp S3 object and transcription job
    let _ = s3_client
        .delete_object()
        .bucket(s3_bucket)
        .key(&s3_key)
        .send()
        .await;
    let _ = transcribe_client
        .delete_transcription_job()
        .transcription_job_name(&job_name)
        .send()
        .await;

    Ok(text)
}

// ── Inventory registrations ─────────────────────────────────────────────────

// Streaming backend
inventory::submit! { ProviderRegistration {
    backend_id: "aws-transcribe-streaming",
    factory: || Box::new(AwsTranscribeStreamingBackend),
}}

// Batch backend
inventory::submit! { ProviderRegistration {
    backend_id: "aws-transcribe-batch",
    factory: || Box::new(AwsTranscribeBatchBackend),
}}

// Streaming preset — no S3 needed, ideal for short dictation
inventory::submit! { ProviderPreset {
    id: "aws-transcribe", display_name: "AWS Transcribe",
    base_url: "https://transcribestreaming.us-east-1.amazonaws.com",
    backend_id: "aws-transcribe-streaming",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #ff9900, #ec7211)",
    default_asr_models: &["default"],
    default_llm_models: &[],
    extra_fields: &[
        PresetField {
            id: "access_key",
            label: "Access Key ID",
            field_type: FieldType::Text,
            required: true,
            placeholder: "AKIA...",
            default_value: "",
            options: &[],
            sensitive: true,
        },
        PresetField {
            id: "secret_key",
            label: "Secret Access Key",
            field_type: FieldType::Password,
            required: true,
            placeholder: "",
            default_value: "",
            options: &[],
            sensitive: true,
        },
        PresetField {
            id: "region",
            label: "Region",
            field_type: FieldType::Select,
            required: true,
            placeholder: "",
            default_value: "us-east-1",
            options: AWS_REGIONS,
            sensitive: false,
        },
    ],
    hidden_fields: &["api_key", "base_url"],
}}

// Batch preset — requires S3 bucket, better for longer audio
inventory::submit! { ProviderPreset {
    id: "aws-transcribe-batch", display_name: "AWS Transcribe (Batch)",
    base_url: "https://transcribe.us-east-1.amazonaws.com",
    backend_id: "aws-transcribe-batch",
    supports_asr: true, supports_llm: false,
    gradient: "linear-gradient(135deg, #ff9900, #d45b07)",
    default_asr_models: &["default"],
    default_llm_models: &[],
    extra_fields: &[
        PresetField {
            id: "access_key",
            label: "Access Key ID",
            field_type: FieldType::Text,
            required: true,
            placeholder: "AKIA...",
            default_value: "",
            options: &[],
            sensitive: true,
        },
        PresetField {
            id: "secret_key",
            label: "Secret Access Key",
            field_type: FieldType::Password,
            required: true,
            placeholder: "",
            default_value: "",
            options: &[],
            sensitive: true,
        },
        PresetField {
            id: "region",
            label: "Region",
            field_type: FieldType::Select,
            required: true,
            placeholder: "",
            default_value: "us-east-1",
            options: AWS_REGIONS,
            sensitive: false,
        },
        PresetField {
            id: "s3_bucket",
            label: "S3 Bucket",
            field_type: FieldType::Text,
            required: true,
            placeholder: "my-transcribe-bucket",
            default_value: "",
            options: &[],
            sensitive: false,
        },
    ],
    hidden_fields: &["api_key", "base_url"],
}}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aws_language_code_two_letter() {
        assert_eq!(aws_language_code("fr"), "fr-FR");
        assert_eq!(aws_language_code("en"), "en-US");
        assert_eq!(aws_language_code("es"), "es-US");
    }

    #[test]
    fn aws_language_code_passthrough() {
        assert_eq!(aws_language_code("fr-CA"), "fr-CA");
        assert_eq!(aws_language_code("en_GB"), "en-GB");
    }

    #[test]
    fn aws_language_code_auto() {
        assert_eq!(aws_language_code("auto"), "en-US");
    }

    #[test]
    fn aws_language_code_unknown() {
        assert_eq!(aws_language_code("xx"), "xx-XX");
    }

    #[test]
    fn aws_config_missing_access_key() {
        let p = Provider {
            id: "p1".into(),
            name: "Test".into(),
            kind: "aws-transcribe".into(),
            url: String::new(),
            api_key: String::new(),
            allow_insecure: false,
            cached_models: vec![],
            supports_asr: true,
            supports_llm: false,
            api_format: None,
            extra: Default::default(),
        };
        assert!(aws_config(&p).is_err());
    }
}
