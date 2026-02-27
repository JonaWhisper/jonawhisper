use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "WhisperDictate";
const PREFS_FILE: &str = "preferences.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub text: String,
    pub timestamp: u64,
}

// -- Grouped state --

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

/// Model download progress.
pub struct DownloadState {
    pub model_id: Option<String>,
    pub progress: f64,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self { model_id: None, progress: 0.0 }
    }
}

// -- Main AppState --

pub struct AppState {
    pub runtime: Mutex<RuntimeState>,
    pub download: Mutex<DownloadState>,
    pub settings: Mutex<Preferences>,
    pub history: Mutex<Vec<HistoryEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiServerConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_llm_provider(),
            api_url: String::new(),
            api_key: String::new(),
            model: String::new(),
        }
    }
}

fn default_llm_provider() -> String { "openai".to_string() }

/// Persistent preferences (subset of AppState that survives restarts).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Preferences {
    #[serde(default = "default_model_id")]
    pub selected_model_id: String,
    #[serde(default = "default_language")]
    pub selected_language: String,
    #[serde(default = "default_true")]
    pub post_processing_enabled: bool,
    #[serde(default = "default_hotkey")]
    pub hotkey_option: String,
    #[serde(default)]
    pub selected_input_device_uid: Option<String>,
    #[serde(default)]
    pub api_servers: Vec<ApiServerConfig>,
    #[serde(default = "default_auto")]
    pub app_locale: String,
    #[serde(default = "default_true")]
    pub hallucination_filter_enabled: bool,
    #[serde(default = "default_cancel_shortcut")]
    pub cancel_shortcut: String,
    #[serde(default = "default_recording_mode")]
    pub recording_mode: String,
    #[serde(default)]
    pub llm_config: LlmConfig,
}

fn default_model_id() -> String { "whisper:large-v3-turbo".to_string() }
fn default_language() -> String { "auto".to_string() }
fn default_true() -> bool { true }
fn default_hotkey() -> String { "right_command".to_string() }
fn default_auto() -> String { "auto".to_string() }
fn default_cancel_shortcut() -> String { "escape".to_string() }
fn default_recording_mode() -> String { "push_to_talk".to_string() }

/// Config directory: ~/Library/Application Support/WhisperDictate/ (macOS)
/// or %APPDATA%/WhisperDictate/ (Windows).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join(APP_DIR_NAME)
}

fn prefs_path() -> PathBuf {
    config_dir().join(PREFS_FILE)
}

impl Preferences {
    pub fn load() -> Self {
        let path = prefs_path();
        match std::fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = prefs_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            match std::fs::write(&path, &data) {
                Ok(()) => log::info!("save_preferences: written to {}", path.display()),
                Err(e) => log::error!("save_preferences: FAILED to write {}: {}", path.display(), e),
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        let prefs = Preferences::load();
        Self {
            runtime: Mutex::new(RuntimeState::default()),
            download: Mutex::new(DownloadState::default()),
            settings: Mutex::new(prefs),
            history: Mutex::new(Vec::new()),
        }
    }
}

impl AppState {
    /// Save current preferences to disk.
    pub fn save_preferences(&self) {
        let settings = self.settings.lock().unwrap();
        log::info!("save_preferences: post_processing={}, hallucination_filter={}",
            settings.post_processing_enabled,
            settings.hallucination_filter_enabled,
        );
        settings.save();
    }

    pub fn enqueue(&self, path: PathBuf) -> usize {
        let mut rt = self.runtime.lock().unwrap();
        rt.queue.push_back(path);
        rt.queue.len()
    }

    pub fn dequeue(&self) -> Option<PathBuf> {
        self.runtime.lock().unwrap().queue.pop_front()
    }

    pub fn queue_count(&self) -> usize {
        self.runtime.lock().unwrap().queue.len()
    }

    /// Runtime state only — no user settings (those come from get_settings).
    pub fn to_frontend_json(&self) -> serde_json::Value {
        let rt = self.runtime.lock().unwrap();
        let dl = self.download.lock().unwrap();
        serde_json::json!({
            "is_recording": rt.is_recording,
            "is_transcribing": rt.is_transcribing,
            "queue_count": rt.queue.len(),
            "downloading_model_id": dl.model_id,
            "download_progress": dl.progress,
        })
    }

    pub fn add_history(&self, text: String) {
        let mut history = self.history.lock().unwrap();
        let entry = HistoryEntry {
            text,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        history.insert(0, entry);
        if history.len() > 20 {
            history.truncate(20);
        }
    }
}
