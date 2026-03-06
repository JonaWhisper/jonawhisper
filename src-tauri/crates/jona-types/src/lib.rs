use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};

// -- Inference context abstractions --

/// Trait for inference contexts that cache a loaded model.
pub trait HasModelId: Send {
    fn model_id(&self) -> &str;
}

/// A thread-safe slot for a cached inference context.
/// Wraps `Mutex<Option<T>>` with convenience helpers.
pub struct ContextSlot<T>(Mutex<Option<T>>);

impl<T> ContextSlot<T> {
    pub fn empty() -> Self {
        Self(Mutex::new(None))
    }

    pub fn lock(&self) -> MutexGuard<'_, Option<T>> {
        self.0.lock().unwrap()
    }

    pub fn invalidate(&self) {
        *self.0.lock().unwrap() = None;
    }
}

impl<T: HasModelId> ContextSlot<T> {
    /// Ensure the slot contains a context for `model_id`, loading via `loader` if needed.
    /// Returns the locked guard (guaranteed `Some` after success).
    pub fn get_or_load<E>(
        &self,
        model_id: &str,
        loader: impl FnOnce() -> Result<T, E>,
    ) -> Result<MutexGuard<'_, Option<T>>, E> {
        let mut guard = self.0.lock().unwrap();
        if guard.as_ref().map_or(true, |ctx| ctx.model_id() != model_id) {
            *guard = Some(loader()?);
        }
        Ok(guard)
    }
}

/// Generate a context group struct with `new()` and `invalidate_all()`.
#[macro_export]
macro_rules! context_group {
    ($name:ident { $($field:ident : $ctx:ty),* $(,)? }) => {
        pub struct $name { $(pub $field: $crate::ContextSlot<$ctx>,)* }
        impl $name {
            pub fn new() -> Self { Self { $($field: $crate::ContextSlot::empty(),)* } }
            pub fn invalidate_all(&self) { $(self.$field.invalidate();)* }
        }
    };
}

// -- History --

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Model ID for cleanup: "bert-punctuation:*", "llama:*", or "cloud"
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
