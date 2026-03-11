use jona_types::{
    CloudProvider, FieldType, PresetField, Provider, ProviderError, ProviderPreset,
    ProviderRegistration, TranscriptionResult,
};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::LazyLock;

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

/// Pre-built async reqwest client with timeout for downloading transcripts.
static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

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

/// Run an async future from a sync context, using the current Tokio handle if available,
/// or creating a temporary runtime as fallback.
fn run_async<F: std::future::Future>(fut: F) -> F::Output {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.block_on(fut)
    } else {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create Tokio runtime for AWS Transcribe");
        rt.block_on(fut)
    }
}

/// Read a WAV file and extract raw signed 16-bit PCM bytes + sample rate.
/// Handles any WAV format (16-bit, 32-bit float, etc.) by converting to i16 PCM.
fn read_wav_pcm16(audio_path: &Path) -> Result<(Vec<u8>, u32), ProviderError> {
    let reader = hound::WavReader::open(audio_path)
        .map_err(|e| ProviderError::InvalidResponse(format!("Failed to read WAV: {e}")))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    let pcm_i16: Vec<i16> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => reader
            .into_samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ProviderError::InvalidResponse(format!("WAV decode error: {e}")))?,
        (hound::SampleFormat::Int, bps) => {
            let shift = bps.saturating_sub(16);
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| (v >> shift) as i16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ProviderError::InvalidResponse(format!("WAV decode error: {e}")))?
        }
        (hound::SampleFormat::Float, _) => reader
            .into_samples::<f32>()
            .map(|s| s.map(|v| (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ProviderError::InvalidResponse(format!("WAV decode error: {e}")))?,
    };

    // Convert i16 samples to little-endian bytes
    let pcm_bytes: Vec<u8> = pcm_i16.iter().flat_map(|s| s.to_le_bytes()).collect();
    Ok((pcm_bytes, sample_rate))
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
        let (pcm_data, sample_rate) = read_wav_pcm16(audio_path)?;
        let lang_code = aws_language_code(language);

        let result = run_async(async {
            streaming_transcribe(&config, &pcm_data, sample_rate as i32, &lang_code).await
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
        .map_err(|_| {
            ProviderError::NotConfigured(format!(
                "Unsupported AWS Transcribe language code: {language_code}"
            ))
        })?;

    // Build audio events — must collect because EventStreamSender requires 'static
    let events: Vec<_> = pcm_data
        .chunks(8192)
        .map(|chunk| {
            Ok(AudioStream::AudioEvent(
                AudioEvent::builder()
                    .audio_chunk(Blob::new(chunk.to_vec()))
                    .build(),
            ))
        })
        .collect();
    let audio_stream = futures_util::stream::iter(events);

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
                        // Take only the first (best) alternative per result
                        if let Some(alt) = result.alternatives().first() {
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

        let result = run_async(async {
            batch_transcribe(&config, &audio_bytes, &s3_bucket, &lang_code).await
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
    let job_name = format!("jona-{}", job_id);

    // Cleanup helper — always delete S3 object and transcription job
    let cleanup = |s3: &aws_sdk_s3::Client, tc: &aws_sdk_transcribe::Client, bucket: &str, key: &str, name: &str| {
        let s3 = s3.clone();
        let tc = tc.clone();
        let bucket = bucket.to_string();
        let key = key.to_string();
        let name = name.to_string();
        async move {
            let _ = s3.delete_object().bucket(&bucket).key(&key).send().await;
            let _ = tc.delete_transcription_job().transcription_job_name(&name).send().await;
        }
    };

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
        .map_err(|_| {
            ProviderError::NotConfigured(format!(
                "Unsupported AWS Transcribe language code: {language_code}"
            ))
        })?;

    if let Err(e) = transcribe_client
        .start_transcription_job()
        .transcription_job_name(&job_name)
        .language_code(lang)
        .media_format(aws_sdk_transcribe::types::MediaFormat::Wav)
        .media(Media::builder().media_file_uri(&s3_uri).build())
        .send()
        .await
    {
        cleanup(&s3_client, &transcribe_client, s3_bucket, &s3_key, &job_name).await;
        return Err(ProviderError::Http(format!("StartTranscriptionJob failed: {e}")));
    }

    // 3. Poll until complete (max ~120s)
    let mut transcript_uri = String::new();
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let resp = match transcribe_client
            .get_transcription_job()
            .transcription_job_name(&job_name)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                cleanup(&s3_client, &transcribe_client, s3_bucket, &s3_key, &job_name).await;
                return Err(ProviderError::Http(format!("GetTranscriptionJob failed: {e}")));
            }
        };

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
                    cleanup(&s3_client, &transcribe_client, s3_bucket, &s3_key, &job_name).await;
                    return Err(ProviderError::Api {
                        status: 500,
                        body: format!("Transcription failed: {reason}"),
                    });
                }
                _ => continue,
            }
        }
    }

    // 4. Download transcript JSON from the result URI (with timeout + status check)
    let text = if !transcript_uri.is_empty() {
        let resp = match HTTP_CLIENT.get(&transcript_uri).send().await {
            Ok(r) => r,
            Err(e) => {
                cleanup(&s3_client, &transcribe_client, s3_bucket, &s3_key, &job_name).await;
                return Err(ProviderError::Http(format!("Failed to download transcript: {e}")));
            }
        };

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            cleanup(&s3_client, &transcribe_client, s3_bucket, &s3_key, &job_name).await;
            return Err(ProviderError::Api {
                status,
                body: format!("Transcript download returned HTTP {status}"),
            });
        }

        match resp.json::<serde_json::Value>().await {
            Ok(json) => json
                .pointer("/results/transcripts/0/transcript")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            Err(e) => {
                cleanup(&s3_client, &transcribe_client, s3_bucket, &s3_key, &job_name).await;
                return Err(ProviderError::InvalidResponse(e.to_string()));
            }
        }
    } else {
        // Timeout — cleanup before returning error
        cleanup(&s3_client, &transcribe_client, s3_bucket, &s3_key, &job_name).await;
        return Err(ProviderError::Http(
            "Transcription job timed out".to_string(),
        ));
    };

    // 5. Cleanup: delete temp S3 object and transcription job
    cleanup(&s3_client, &transcribe_client, s3_bucket, &s3_key, &job_name).await;

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
