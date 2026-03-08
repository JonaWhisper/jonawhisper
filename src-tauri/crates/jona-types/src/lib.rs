pub mod engine;
pub use engine::*;

use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

// -- Dynamic context map (plug-and-play engine contexts) --

struct ContextEntry {
    key: String,
    ctx: Box<dyn Any + Send>,
}

/// Type-erased storage for engine inference contexts.
/// Replaces typed `ContextSlot<T>` and `context_group!` with a single map
/// keyed by engine_id. Contexts are lazily loaded and invalidated when
/// the context_key changes (different model, different gpu_mode, etc.).
pub struct ContextMap {
    entries: Mutex<HashMap<String, ContextEntry>>,
}

impl Default for ContextMap {
    fn default() -> Self {
        Self { entries: Mutex::new(HashMap::new()) }
    }
}

impl ContextMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get-or-load the context for `engine_id`, then run `action` on it.
    /// If the stored context_key differs from the requested one, the old context
    /// is dropped and `loader` creates a fresh one.
    ///
    /// The lock is NOT held during `loader()` or `action()` — only brief lock
    /// acquisitions to check/insert/remove entries. This prevents engines from
    /// blocking each other during inference.
    pub fn run_with<R>(
        &self,
        engine_id: &str,
        context_key: &str,
        loader: impl FnOnce() -> Result<Box<dyn Any + Send>, EngineError>,
        action: impl FnOnce(&mut dyn Any) -> Result<R, EngineError>,
    ) -> Result<R, EngineError> {
        // Phase 1: check if load needed (short lock)
        let needs_load = {
            let map = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            map.get(engine_id).is_none_or(|e| e.key != context_key)
        };

        // Phase 2: load outside lock
        if needs_load {
            log::info!("ContextMap: loading context for engine={} key={}", engine_id, context_key);
            let start = std::time::Instant::now();
            let ctx = loader()?;
            log::info!("ContextMap: loaded engine={} in {:.1}s", engine_id, start.elapsed().as_secs_f64());
            let mut map = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(engine_id.to_string(), ContextEntry {
                key: context_key.to_string(),
                ctx,
            });
        }

        // Phase 3: remove entry → run action without lock → insert back
        let mut entry = {
            let mut map = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            map.remove(engine_id)
                .ok_or_else(|| EngineError::LaunchFailed("context disappeared".into()))?
        };

        let result = action(&mut *entry.ctx);

        // Re-insert even on error (context is still valid)
        let mut map = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(engine_id.to_string(), entry);

        result
    }

    /// Drop all cached contexts (e.g. on model deletion).
    pub fn invalidate_all(&self) {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Drop the context for a specific engine.
    pub fn invalidate(&self, engine_id: &str) {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).remove(engine_id);
    }
}

// -- History --

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub text: String,
    pub timestamp: u64,
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub cleanup_model_id: String,
    #[serde(default)]
    pub hallucination_filter: bool,
    #[serde(default)]
    pub vad_trimmed: bool,
    #[serde(default)]
    pub punctuation_model_id: String,
    #[serde(default)]
    pub spellcheck: bool,
    #[serde(default)]
    pub disfluency_removal: bool,
    #[serde(default)]
    pub itn: bool,
}

// -- Typed settings enums --

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    #[default]
    PushToTalk,
    Toggle,
}

impl RecordingMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "toggle" => Self::Toggle,
            _ => Self::PushToTalk,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuMode {
    #[default]
    Auto,
    Gpu,
    Cpu,
}

impl GpuMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "gpu" => Self::Gpu,
            "cpu" => Self::Cpu,
            _ => Self::Auto,
        }
    }
}

// -- Grouped state --

/// Atomic flags for hot-path polling (avoids locking RuntimeState).
#[derive(Default)]
pub struct AudioFlags {
    recording: AtomicBool,
    mic_testing: AtomicBool,
}

impl AudioFlags {
    pub fn is_active(&self) -> bool {
        self.recording.load(std::sync::atomic::Ordering::Relaxed)
            || self.mic_testing.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn set_recording(&self, v: bool) {
        self.recording.store(v, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn is_recording(&self) -> bool {
        self.recording.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn set_mic_testing(&self, v: bool) {
        self.mic_testing.store(v, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn is_mic_testing(&self) -> bool {
        self.mic_testing.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Volatile runtime state (recording lifecycle, queue).
#[derive(Default)]
pub struct RuntimeState {
    pub is_recording: bool,
    pub is_transcribing: bool,
    pub queue: VecDeque<PathBuf>,
    pub transcription_cancelled: bool,
    pub last_paste_had_content: bool,
    pub mic_testing: bool,
}

/// Per-model download state.
pub struct ActiveDownload {
    pub progress: f64,
    pub cancel_requested: Arc<AtomicBool>,
    pub delete_partial: Arc<AtomicBool>,
}

/// All active model downloads (keyed by model ID).
#[derive(Default)]
pub struct DownloadState {
    pub active: HashMap<String, ActiveDownload>,
}

// -- Provider --

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Custom,
    Groq,
    Cerebras,
    Gemini,
    Mistral,
    Fireworks,
    Together,
    DeepSeek,
}

impl ProviderKind {
    pub fn is_anthropic_format(&self) -> bool {
        matches!(self, Self::Anthropic)
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::OpenAI => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Custom => "Custom",
            Self::Groq => "Groq",
            Self::Cerebras => "Cerebras",
            Self::Gemini => "Google Gemini",
            Self::Mistral => "Mistral",
            Self::Fireworks => "Fireworks AI",
            Self::Together => "Together AI",
            Self::DeepSeek => "DeepSeek",
        }
    }

    /// Whether this provider kind supports ASR (audio transcription).
    pub fn supports_asr(&self) -> bool {
        matches!(self, Self::OpenAI | Self::Groq | Self::Fireworks | Self::Together | Self::Custom)
    }

    /// Whether this provider kind supports LLM (chat completions).
    pub fn supports_llm(&self) -> bool {
        matches!(self, Self::OpenAI | Self::Anthropic | Self::Groq | Self::Cerebras
            | Self::Gemini | Self::Mistral | Self::Together | Self::DeepSeek | Self::Custom)
    }

    /// Canonical base URL for known providers (includes version path).
    /// Returns None for Custom — use provider.url instead.
    pub fn base_url(&self) -> Option<&'static str> {
        match self {
            Self::OpenAI => Some("https://api.openai.com/v1"),
            Self::Anthropic => Some("https://api.anthropic.com/v1"),
            Self::Groq => Some("https://api.groq.com/openai/v1"),
            Self::Cerebras => Some("https://api.cerebras.ai/v1"),
            Self::Gemini => Some("https://generativelanguage.googleapis.com/v1beta/openai"),
            Self::Mistral => Some("https://api.mistral.ai/v1"),
            Self::Fireworks => Some("https://api.fireworks.ai/inference/v1"),
            Self::Together => Some("https://api.together.xyz/v1"),
            Self::DeepSeek => Some("https://api.deepseek.com/v1"),
            Self::Custom => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub allow_insecure: bool,
    #[serde(default)]
    pub cached_models: Vec<String>,
    #[serde(default = "default_true")]
    pub supports_asr: bool,
    #[serde(default = "default_true")]
    pub supports_llm: bool,
}

impl Provider {
    /// Whether this provider supports ASR transcription.
    /// Known providers: derived from kind. Custom: uses explicit field.
    pub fn has_asr(&self) -> bool {
        if self.kind == ProviderKind::Custom { self.supports_asr } else { self.kind.supports_asr() }
    }

    /// Whether this provider supports LLM chat completions.
    /// Known providers: derived from kind. Custom: uses explicit field.
    pub fn has_llm(&self) -> bool {
        if self.kind == ProviderKind::Custom { self.supports_llm } else { self.kind.supports_llm() }
    }

    /// Resolved base URL: preset URL for known providers, stored URL for Custom.
    pub fn base_url(&self) -> &str {
        self.kind.base_url().unwrap_or_else(|| self.url.trim_end_matches('/'))
    }

    /// Validate the provider URL scheme. Returns Err if Custom provider uses HTTP
    /// without `allow_insecure` enabled. Known providers always use HTTPS.
    pub fn validate_url(&self) -> Result<(), String> {
        if self.kind != ProviderKind::Custom {
            return Ok(());
        }
        let url = self.base_url();
        if url.starts_with("http://") && !self.allow_insecure {
            return Err("HTTP is not allowed for custom providers. Enable 'Allow insecure connection' to use HTTP.".to_string());
        }
        Ok(())
    }

    /// Return a masked version of the API key for display (e.g. "••••abcd").
    pub fn masked_api_key(&self) -> String {
        if self.api_key.is_empty() {
            return String::new();
        }
        if self.api_key.len() > 4 {
            format!("\u{2022}\u{2022}\u{2022}\u{2022}{}", &self.api_key[self.api_key.len() - 4..])
        } else {
            "\u{2022}\u{2022}\u{2022}\u{2022}".to_string()
        }
    }
}

// -- Keyring helpers --

const KEYRING_SERVICE: &str = "JonaWhisper";

fn keyring_user(provider_id: &str) -> String {
    format!("provider:{}", provider_id)
}

/// Store an API key in the OS keychain.
pub fn keyring_store(provider_id: &str, api_key: &str) {
    if api_key.is_empty() {
        return;
    }
    match keyring::Entry::new(KEYRING_SERVICE, &keyring_user(provider_id)) {
        Ok(entry) => {
            if let Err(e) = entry.set_password(api_key) {
                log::error!("keyring: failed to store key for {}: {}", provider_id, e);
            }
        }
        Err(e) => log::error!("keyring: failed to create entry for {}: {}", provider_id, e),
    }
}

/// Load an API key from the OS keychain. Returns empty string on failure.
pub fn keyring_load(provider_id: &str) -> String {
    match keyring::Entry::new(KEYRING_SERVICE, &keyring_user(provider_id)) {
        Ok(entry) => entry.get_password().unwrap_or_default(),
        Err(e) => {
            log::warn!("keyring: failed to create entry for {}: {}", provider_id, e);
            String::new()
        }
    }
}

/// Delete an API key from the OS keychain.
pub fn keyring_delete(provider_id: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &keyring_user(provider_id)) {
        let _ = entry.delete_credential();
    }
}

/// Populate empty api_key fields from the OS keychain.
pub fn load_api_keys_from_keyring(providers: &mut [Provider]) {
    for provider in providers.iter_mut() {
        if provider.api_key.is_empty() {
            provider.api_key = keyring_load(&provider.id);
        }
    }
}

// -- Paths --

const APP_DIR_NAME: &str = "JonaWhisper";
pub const PREFS_FILE: &str = "preferences.json";
pub const HISTORY_DB: &str = "history.db";
pub const HISTORY_JSON_LEGACY: &str = "history.json";

/// Config directory: ~/Library/Application Support/JonaWhisper/ (macOS)
/// or %APPDATA%/JonaWhisper/ (Windows).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join(APP_DIR_NAME)
}

/// Model storage: ~/Library/Application Support/JonaWhisper/models/
pub fn models_dir() -> PathBuf {
    config_dir().join("models")
}

/// Storage directory for a specific engine's models.
pub fn engine_storage_dir(engine_name: &str) -> String {
    models_dir().join(engine_name).to_string_lossy().to_string()
}

pub fn prefs_path() -> PathBuf {
    config_dir().join(PREFS_FILE)
}

// -- Preferences --

/// Persistent preferences (subset of AppState that survives restarts).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Preferences {
    #[serde(default, rename = "_version")]
    pub schema_version: u32,
    #[serde(default = "default_model_id")]
    pub selected_model_id: String,
    #[serde(default = "default_language")]
    pub selected_language: String,
    #[serde(default = "default_hotkey")]
    pub hotkey_option: String,
    #[serde(default)]
    pub selected_input_device_uid: Option<String>,
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default = "default_auto")]
    pub app_locale: String,
    #[serde(default = "default_true")]
    pub hallucination_filter_enabled: bool,
    #[serde(default = "default_cancel_shortcut")]
    pub cancel_shortcut: String,
    #[serde(default)]
    pub recording_mode: RecordingMode,
    #[serde(default)]
    pub text_cleanup_enabled: bool,
    /// Model ID for punctuation engine (PCS/BERT), runs before cleanup
    #[serde(default)]
    pub punctuation_model_id: String,
    /// Model ID for cleanup: correction (T5), LLM (llama), or "cloud:*"
    #[serde(default)]
    pub cleanup_model_id: String,
    #[serde(default)]
    pub llm_provider_id: String,
    #[serde(default)]
    pub llm_model: String,
    #[serde(default = "default_asr_cloud_model")]
    pub asr_cloud_model: String,
    #[serde(default)]
    pub gpu_mode: GpuMode,
    #[serde(default = "default_llm_max_tokens")]
    pub llm_max_tokens: u32,
    #[serde(default)]
    pub audio_ducking_enabled: bool,
    #[serde(default = "default_ducking_level")]
    pub audio_ducking_level: f32,
    #[serde(default = "default_true")]
    pub vad_enabled: bool,
    #[serde(default = "default_true")]
    pub disfluency_removal_enabled: bool,
    #[serde(default = "default_true")]
    pub itn_enabled: bool,
    #[serde(default)]
    pub spellcheck_enabled: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

pub fn default_model_id() -> String { "whisper:large-v3-turbo-q8".to_string() }
fn default_language() -> String { "auto".to_string() }
fn default_true() -> bool { true }
fn default_hotkey() -> String { "right_command".to_string() }
fn default_auto() -> String { "auto".to_string() }
fn default_cancel_shortcut() -> String { "escape".to_string() }
fn default_asr_cloud_model() -> String { "whisper-1".to_string() }
fn default_llm_max_tokens() -> u32 { 4096 }
fn default_ducking_level() -> f32 { 0.8 }
fn default_theme() -> String { "system".to_string() }

impl Preferences {
    pub fn save(&self) {
        let path = prefs_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Clone and strip API keys — they live in the OS keychain, not on disk
        let mut prefs_for_disk = self.clone();
        for provider in &mut prefs_for_disk.providers {
            provider.api_key.clear();
        }
        if let Ok(data) = serde_json::to_string_pretty(&prefs_for_disk) {
            match std::fs::write(&path, &data) {
                Ok(()) => log::info!("save_preferences: written to {}", path.display()),
                Err(e) => log::error!("save_preferences: FAILED to write {}: {}", path.display(), e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Storage paths (app must find its data across restarts) --

    #[test]
    fn downloaded_models_stored_in_dedicated_directory() {
        let path = models_dir();
        assert!(path.ends_with("models"),
            "Models dir should end with 'models' for organized storage, got: {}", path.display());
    }

    #[test]
    fn each_engine_gets_isolated_storage() {
        // Different engines must not clobber each other's model files.
        let dir = engine_storage_dir("whisper");
        assert!(dir.contains("whisper"), "Engine storage must include engine name");
        assert!(dir.contains("models"), "Engine storage must be under models dir");
    }

    #[test]
    fn config_stored_under_app_name() {
        // Preferences file must be under JonaWhisper config dir,
        // not polluting other apps' config.
        let path = config_dir();
        assert!(path.to_string_lossy().contains("JonaWhisper"));
    }

    #[test]
    fn preferences_file_has_stable_name() {
        // The preferences filename must be stable across app versions
        // so settings persist through updates.
        let path = prefs_path();
        assert!(path.ends_with(PREFS_FILE));
    }

    // -- ASRModel --

    #[test]
    fn new_model_is_not_downloaded_by_default() {
        // A freshly created model must not appear as downloaded until
        // files actually exist on disk.
        let model = ASRModel::default();
        assert!(!model.is_downloaded(), "Default model should not be considered downloaded");
    }

    #[test]
    fn asr_model_is_downloaded_remote_api() {
        let model = ASRModel {
            download_type: DownloadType::RemoteAPI,
            ..Default::default()
        };
        assert!(model.is_downloaded());
    }

    #[test]
    fn asr_model_is_downloaded_system() {
        let model = ASRModel {
            download_type: DownloadType::System,
            ..Default::default()
        };
        assert!(model.is_downloaded());
    }

    #[test]
    fn asr_model_is_downloaded_single_file_missing() {
        let model = ASRModel {
            storage_dir: "/tmp/jona_test_nonexistent".to_string(),
            filename: "model.bin".to_string(),
            download_type: DownloadType::SingleFile,
            ..Default::default()
        };
        assert!(!model.is_downloaded());
    }

    #[test]
    fn asr_model_is_downloaded_single_file_exists() {
        let dir = std::env::temp_dir().join("jona_test_is_downloaded");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("model.bin");
        std::fs::write(&file, b"dummy").unwrap();

        let model = ASRModel {
            storage_dir: dir.to_string_lossy().to_string(),
            filename: "model.bin".to_string(),
            download_type: DownloadType::SingleFile,
            ..Default::default()
        };
        assert!(model.is_downloaded());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn asr_model_is_downloaded_multifile_with_marker() {
        let dir = std::env::temp_dir().join("jona_test_multifile");
        let model_dir = dir.join("my_model");
        let _ = std::fs::create_dir_all(&model_dir);

        let model = ASRModel {
            storage_dir: dir.to_string_lossy().to_string(),
            filename: "my_model".to_string(),
            download_marker: Some(".complete".to_string()),
            download_type: DownloadType::MultiFile { files: vec![] },
            ..Default::default()
        };

        // Without marker file
        assert!(!model.is_downloaded());

        // With marker file
        std::fs::write(model_dir.join(".complete"), "").unwrap();
        assert!(model.is_downloaded());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn asr_model_is_recommended_for() {
        // No recommended_for => not recommended
        let model = ASRModel::default();
        assert!(!model.is_recommended_for("fr"));

        // Empty vec => recommended for all
        let model = ASRModel {
            recommended_for: Some(vec![]),
            ..Default::default()
        };
        assert!(model.is_recommended_for("fr"));
        assert!(model.is_recommended_for("en"));

        // Specific languages
        let model = ASRModel {
            recommended_for: Some(vec!["fr".to_string()]),
            ..Default::default()
        };
        assert!(model.is_recommended_for("fr"));
        assert!(!model.is_recommended_for("en"));
        assert!(model.is_recommended_for("auto")); // auto always matches
    }

    #[test]
    fn asr_model_local_path() {
        let model = ASRModel {
            storage_dir: "/tmp/test_storage".to_string(),
            filename: "model.onnx".to_string(),
            ..Default::default()
        };
        let path = model.local_path();
        assert_eq!(path, PathBuf::from("/tmp/test_storage/model.onnx"));
    }

    // -- EngineCategory --

    #[test]
    fn engine_category_equality() {
        assert_eq!(EngineCategory::ASR, EngineCategory::ASR);
        assert_ne!(EngineCategory::ASR, EngineCategory::LLM);
        assert_ne!(EngineCategory::Punctuation, EngineCategory::Correction);
    }

    #[test]
    fn engine_category_serde_roundtrip() {
        let categories = vec![
            EngineCategory::ASR,
            EngineCategory::LLM,
            EngineCategory::Punctuation,
            EngineCategory::Correction,
            EngineCategory::SpellCheck,
            EngineCategory::LanguageModel,
        ];
        for cat in &categories {
            let json = serde_json::to_string(cat).unwrap();
            let deserialized: EngineCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(*cat, deserialized);
        }
    }

    #[test]
    fn engine_category_serde_rename() {
        assert_eq!(serde_json::to_string(&EngineCategory::SpellCheck).unwrap(), "\"spellcheck\"");
        assert_eq!(serde_json::to_string(&EngineCategory::ASR).unwrap(), "\"asr\"");
        assert_eq!(serde_json::to_string(&EngineCategory::LanguageModel).unwrap(), "\"languagemodel\"");
    }

    // -- EngineError --

    #[test]
    fn engine_error_display() {
        let e = EngineError::ModelNotFound("/path/to/model".into());
        assert!(e.to_string().contains("/path/to/model"));

        let e = EngineError::LaunchFailed("bad config".into());
        assert!(e.to_string().contains("bad config"));

        let e = EngineError::ApiError("rate limited".into());
        assert!(e.to_string().contains("rate limited"));
    }

    #[test]
    fn engine_error_serializes_as_string() {
        let e = EngineError::LaunchFailed("test error".into());
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("test error"));
        // Should serialize as a plain string, not an object
        assert!(json.starts_with('"'));
    }

    // -- Preferences --

    #[test]
    fn preferences_default_values() {
        let prefs = Preferences::default();
        assert_eq!(prefs.schema_version, 0);
        assert!(prefs.selected_model_id.is_empty());
        assert!(prefs.selected_language.is_empty());
        assert!(prefs.hotkey_option.is_empty());
        assert!(!prefs.hallucination_filter_enabled);
        assert_eq!(prefs.recording_mode, RecordingMode::PushToTalk);
        assert_eq!(prefs.gpu_mode, GpuMode::Auto);
        assert!(!prefs.text_cleanup_enabled);
        assert!(!prefs.vad_enabled);
    }

    #[test]
    fn preferences_serde_with_defaults() {
        // Deserializing from empty JSON should apply serde defaults
        let json = "{}";
        let prefs: Preferences = serde_json::from_str(json).unwrap();
        assert_eq!(prefs.selected_model_id, "whisper:large-v3-turbo-q8");
        assert_eq!(prefs.selected_language, "auto");
        assert_eq!(prefs.hotkey_option, "right_command");
        assert!(prefs.hallucination_filter_enabled);
        assert_eq!(prefs.app_locale, "auto");
        assert_eq!(prefs.cancel_shortcut, "escape");
        assert_eq!(prefs.asr_cloud_model, "whisper-1");
        assert_eq!(prefs.llm_max_tokens, 4096);
        assert_eq!(prefs.audio_ducking_level, 0.8);
        assert!(prefs.vad_enabled);
        assert!(prefs.disfluency_removal_enabled);
        assert!(prefs.itn_enabled);
        assert_eq!(prefs.theme, "system");
    }

    #[test]
    fn preferences_serde_roundtrip() {
        let prefs = Preferences {
            schema_version: 5,
            selected_model_id: "whisper:tiny".to_string(),
            selected_language: "fr".to_string(),
            hotkey_option: "left_command".to_string(),
            recording_mode: RecordingMode::Toggle,
            gpu_mode: GpuMode::Cpu,
            llm_max_tokens: 2048,
            audio_ducking_level: 0.5,
            theme: "dark".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let deserialized: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.schema_version, 5);
        assert_eq!(deserialized.selected_model_id, "whisper:tiny");
        assert_eq!(deserialized.selected_language, "fr");
        assert_eq!(deserialized.recording_mode, RecordingMode::Toggle);
        assert_eq!(deserialized.gpu_mode, GpuMode::Cpu);
        assert_eq!(deserialized.llm_max_tokens, 2048);
        assert_eq!(deserialized.audio_ducking_level, 0.5);
        assert_eq!(deserialized.theme, "dark");
    }

    // -- RecordingMode --

    #[test]
    fn recording_mode_parse() {
        assert_eq!(RecordingMode::parse("toggle"), RecordingMode::Toggle);
        assert_eq!(RecordingMode::parse("push_to_talk"), RecordingMode::PushToTalk);
        assert_eq!(RecordingMode::parse("anything_else"), RecordingMode::PushToTalk);
    }

    // -- GpuMode --

    #[test]
    fn gpu_mode_parse() {
        assert_eq!(GpuMode::parse("gpu"), GpuMode::Gpu);
        assert_eq!(GpuMode::parse("cpu"), GpuMode::Cpu);
        assert_eq!(GpuMode::parse("auto"), GpuMode::Auto);
        assert_eq!(GpuMode::parse("unknown"), GpuMode::Auto);
    }

    // -- ProviderKind --

    #[test]
    fn provider_kind_display_name() {
        assert_eq!(ProviderKind::OpenAI.display_name(), "OpenAI");
        assert_eq!(ProviderKind::Anthropic.display_name(), "Anthropic");
        assert_eq!(ProviderKind::Gemini.display_name(), "Google Gemini");
        assert_eq!(ProviderKind::Fireworks.display_name(), "Fireworks AI");
        assert_eq!(ProviderKind::Together.display_name(), "Together AI");
    }

    #[test]
    fn provider_kind_is_anthropic_format() {
        assert!(ProviderKind::Anthropic.is_anthropic_format());
        assert!(!ProviderKind::OpenAI.is_anthropic_format());
        assert!(!ProviderKind::Custom.is_anthropic_format());
    }

    #[test]
    fn provider_kind_supports_asr() {
        assert!(ProviderKind::OpenAI.supports_asr());
        assert!(ProviderKind::Groq.supports_asr());
        assert!(!ProviderKind::Anthropic.supports_asr());
        assert!(!ProviderKind::Gemini.supports_asr());
        assert!(ProviderKind::Custom.supports_asr());
    }

    #[test]
    fn provider_kind_supports_llm() {
        assert!(ProviderKind::OpenAI.supports_llm());
        assert!(ProviderKind::Anthropic.supports_llm());
        assert!(!ProviderKind::Fireworks.supports_llm());
        assert!(ProviderKind::Custom.supports_llm());
    }

    #[test]
    fn provider_kind_base_url() {
        assert_eq!(ProviderKind::OpenAI.base_url(), Some("https://api.openai.com/v1"));
        assert!(ProviderKind::Custom.base_url().is_none());
        assert!(ProviderKind::Groq.base_url().unwrap().starts_with("https://"));
    }

    // -- Provider --

    #[test]
    fn provider_masked_api_key() {
        let p = Provider {
            id: "test".into(), name: "Test".into(), kind: ProviderKind::OpenAI,
            url: String::new(), api_key: "sk-1234567890abcdef".into(),
            allow_insecure: false, cached_models: vec![], supports_asr: true, supports_llm: true,
        };
        let masked = p.masked_api_key();
        assert!(masked.starts_with("\u{2022}\u{2022}\u{2022}\u{2022}"));
        assert!(masked.ends_with("cdef"));
        assert!(!masked.contains("sk-1234"));
    }

    #[test]
    fn provider_masked_api_key_empty() {
        let p = Provider {
            id: "test".into(), name: "Test".into(), kind: ProviderKind::OpenAI,
            url: String::new(), api_key: String::new(),
            allow_insecure: false, cached_models: vec![], supports_asr: true, supports_llm: true,
        };
        assert!(p.masked_api_key().is_empty());
    }

    #[test]
    fn provider_masked_api_key_short() {
        let p = Provider {
            id: "test".into(), name: "Test".into(), kind: ProviderKind::OpenAI,
            url: String::new(), api_key: "abc".into(),
            allow_insecure: false, cached_models: vec![], supports_asr: true, supports_llm: true,
        };
        assert_eq!(p.masked_api_key(), "\u{2022}\u{2022}\u{2022}\u{2022}");
    }

    #[test]
    fn provider_validate_url_known_provider() {
        let p = Provider {
            id: "test".into(), name: "Test".into(), kind: ProviderKind::OpenAI,
            url: String::new(), api_key: String::new(),
            allow_insecure: false, cached_models: vec![], supports_asr: true, supports_llm: true,
        };
        assert!(p.validate_url().is_ok());
    }

    #[test]
    fn provider_validate_url_custom_http_rejected() {
        let p = Provider {
            id: "test".into(), name: "Test".into(), kind: ProviderKind::Custom,
            url: "http://localhost:8080".into(), api_key: String::new(),
            allow_insecure: false, cached_models: vec![], supports_asr: true, supports_llm: true,
        };
        assert!(p.validate_url().is_err());
    }

    #[test]
    fn provider_validate_url_custom_http_allowed() {
        let p = Provider {
            id: "test".into(), name: "Test".into(), kind: ProviderKind::Custom,
            url: "http://localhost:8080".into(), api_key: String::new(),
            allow_insecure: true, cached_models: vec![], supports_asr: true, supports_llm: true,
        };
        assert!(p.validate_url().is_ok());
    }

    #[test]
    fn provider_base_url_known() {
        let p = Provider {
            id: "test".into(), name: "Test".into(), kind: ProviderKind::OpenAI,
            url: String::new(), api_key: String::new(),
            allow_insecure: false, cached_models: vec![], supports_asr: true, supports_llm: true,
        };
        assert_eq!(p.base_url(), "https://api.openai.com/v1");
    }

    #[test]
    fn provider_base_url_custom_trims_trailing_slash() {
        let p = Provider {
            id: "test".into(), name: "Test".into(), kind: ProviderKind::Custom,
            url: "https://my-api.example.com/v1/".into(), api_key: String::new(),
            allow_insecure: false, cached_models: vec![], supports_asr: true, supports_llm: true,
        };
        assert_eq!(p.base_url(), "https://my-api.example.com/v1");
    }

    #[test]
    fn provider_has_asr_known_vs_custom() {
        // Known provider derives from kind
        let p = Provider {
            id: "t".into(), name: "T".into(), kind: ProviderKind::Anthropic,
            url: String::new(), api_key: String::new(),
            allow_insecure: false, cached_models: vec![],
            supports_asr: true, supports_llm: true, // ignored for non-Custom
        };
        assert!(!p.has_asr()); // Anthropic doesn't support ASR

        // Custom uses explicit field
        let p = Provider {
            id: "t".into(), name: "T".into(), kind: ProviderKind::Custom,
            url: "https://x.com".into(), api_key: String::new(),
            allow_insecure: false, cached_models: vec![],
            supports_asr: false, supports_llm: true,
        };
        assert!(!p.has_asr());
    }

    // -- ContextMap --

    #[test]
    fn context_map_run_with_loads_and_caches() {
        let map = ContextMap::new();
        let result = map.run_with(
            "engine1", "key1",
            || Ok(Box::new(42u32) as Box<dyn std::any::Any + Send>),
            |ctx| {
                let val = ctx.downcast_ref::<u32>().unwrap();
                Ok(*val)
            },
        );
        assert_eq!(result.unwrap(), 42);

        // Second call with same key should reuse cached context (loader not called)
        let result = map.run_with(
            "engine1", "key1",
            || panic!("loader should not be called for cached context"),
            |ctx| {
                let val = ctx.downcast_ref::<u32>().unwrap();
                Ok(*val)
            },
        );
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn context_map_invalidates_on_key_change() {
        let map = ContextMap::new();
        map.run_with(
            "engine1", "key1",
            || Ok(Box::new(1u32) as Box<dyn std::any::Any + Send>),
            |_| Ok(()),
        ).unwrap();

        // Different context_key should trigger reload
        let result = map.run_with(
            "engine1", "key2",
            || Ok(Box::new(99u32) as Box<dyn std::any::Any + Send>),
            |ctx| Ok(*ctx.downcast_ref::<u32>().unwrap()),
        );
        assert_eq!(result.unwrap(), 99);
    }

    #[test]
    fn context_map_invalidate_all() {
        let map = ContextMap::new();
        map.run_with(
            "engine1", "key1",
            || Ok(Box::new(1u32) as Box<dyn std::any::Any + Send>),
            |_| Ok(()),
        ).unwrap();

        map.invalidate_all();

        // Should need to reload
        let result = map.run_with(
            "engine1", "key1",
            || Ok(Box::new(77u32) as Box<dyn std::any::Any + Send>),
            |ctx| Ok(*ctx.downcast_ref::<u32>().unwrap()),
        );
        assert_eq!(result.unwrap(), 77);
    }

    #[test]
    fn context_map_invalidate_specific() {
        let map = ContextMap::new();
        map.run_with(
            "engine1", "key1",
            || Ok(Box::new(1u32) as Box<dyn std::any::Any + Send>),
            |_| Ok(()),
        ).unwrap();
        map.run_with(
            "engine2", "key2",
            || Ok(Box::new(2u32) as Box<dyn std::any::Any + Send>),
            |_| Ok(()),
        ).unwrap();

        map.invalidate("engine1");

        // engine1 should reload, engine2 should be cached
        let r1 = map.run_with(
            "engine1", "key1",
            || Ok(Box::new(10u32) as Box<dyn std::any::Any + Send>),
            |ctx| Ok(*ctx.downcast_ref::<u32>().unwrap()),
        );
        assert_eq!(r1.unwrap(), 10);

        let r2 = map.run_with(
            "engine2", "key2",
            || panic!("engine2 should be cached"),
            |ctx| Ok(*ctx.downcast_ref::<u32>().unwrap()),
        );
        assert_eq!(r2.unwrap(), 2);
    }

    // -- AudioFlags --

    #[test]
    fn audio_flags_default_inactive() {
        let flags = AudioFlags::default();
        assert!(!flags.is_active());
        assert!(!flags.is_recording());
        assert!(!flags.is_mic_testing());
    }

    #[test]
    fn audio_flags_recording() {
        let flags = AudioFlags::default();
        flags.set_recording(true);
        assert!(flags.is_recording());
        assert!(flags.is_active());
        flags.set_recording(false);
        assert!(!flags.is_recording());
        assert!(!flags.is_active());
    }

    #[test]
    fn audio_flags_mic_testing() {
        let flags = AudioFlags::default();
        flags.set_mic_testing(true);
        assert!(flags.is_mic_testing());
        assert!(flags.is_active());
        flags.set_mic_testing(false);
        assert!(!flags.is_active());
    }

    #[test]
    fn audio_flags_either_active() {
        let flags = AudioFlags::default();
        flags.set_recording(true);
        flags.set_mic_testing(true);
        assert!(flags.is_active());
        flags.set_recording(false);
        assert!(flags.is_active()); // mic_testing still on
    }

    // -- HistoryEntry --

    #[test]
    fn history_entry_serde_roundtrip() {
        let entry = HistoryEntry {
            text: "Hello world".to_string(),
            timestamp: 1700000000,
            model_id: "whisper:tiny".to_string(),
            language: "en".to_string(),
            cleanup_model_id: String::new(),
            hallucination_filter: true,
            vad_trimmed: false,
            punctuation_model_id: String::new(),
            spellcheck: false,
            disfluency_removal: true,
            itn: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text, "Hello world");
        assert_eq!(deserialized.timestamp, 1700000000);
        assert_eq!(deserialized.model_id, "whisper:tiny");
        assert!(deserialized.hallucination_filter);
        assert!(deserialized.disfluency_removal);
    }

    #[test]
    fn history_entry_serde_defaults() {
        let json = r#"{"text":"test","timestamp":123}"#;
        let entry: HistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.text, "test");
        assert!(entry.model_id.is_empty());
        assert!(!entry.hallucination_filter);
        assert!(!entry.vad_trimmed);
        assert!(!entry.spellcheck);
    }

    // -- DownloadType --

    #[test]
    fn download_type_serde() {
        let dt = DownloadType::SingleFile;
        let json = serde_json::to_string(&dt).unwrap();
        let back: DownloadType = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, DownloadType::SingleFile));

        let dt = DownloadType::RemoteAPI;
        let json = serde_json::to_string(&dt).unwrap();
        let back: DownloadType = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, DownloadType::RemoteAPI));
    }

    // -- common_languages --

    #[test]
    fn common_languages_includes_auto_and_french() {
        let langs = common_languages();
        assert!(langs.len() >= 5);
        assert_eq!(langs[0].code, "auto");
        assert!(langs.iter().any(|l| l.code == "fr"));
        assert!(langs.iter().any(|l| l.code == "en"));
    }
}
