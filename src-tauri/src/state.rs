use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::path::PathBuf;

const PREFS_DIR: &str = ".local/share/whisper-dictate";
const PREFS_FILE: &str = "preferences.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub text: String,
    pub timestamp: u64,
}

pub struct AppState {
    pub is_recording: Mutex<bool>,
    pub is_transcribing: Mutex<bool>,
    pub transcription_queue: Mutex<Vec<PathBuf>>,
    pub downloading_model_id: Mutex<Option<String>>,
    pub download_progress: Mutex<f64>,
    pub transcription_cancelled: Mutex<bool>,
    pub selected_model_id: Mutex<String>,
    pub selected_language: Mutex<String>,
    pub post_processing_enabled: Mutex<bool>,
    pub hotkey_option: Mutex<String>,
    pub selected_input_device_uid: Mutex<Option<String>>,
    pub transcription_history: Mutex<Vec<HistoryEntry>>,
    pub api_servers: Mutex<Vec<ApiServerConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiServerConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub api_key: String,
    pub model: String,
}

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
}

fn default_model_id() -> String { "whisper:large-v3-turbo".to_string() }
fn default_language() -> String { "auto".to_string() }
fn default_true() -> bool { true }
fn default_hotkey() -> String { "right_command".to_string() }

fn prefs_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(PREFS_DIR)
        .join(PREFS_FILE)
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
            let _ = std::fs::write(&path, data);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        let prefs = Preferences::load();
        Self {
            is_recording: Mutex::new(false),
            is_transcribing: Mutex::new(false),
            transcription_queue: Mutex::new(Vec::new()),
            downloading_model_id: Mutex::new(None),
            download_progress: Mutex::new(0.0),
            transcription_cancelled: Mutex::new(false),
            selected_model_id: Mutex::new(prefs.selected_model_id),
            selected_language: Mutex::new(prefs.selected_language),
            post_processing_enabled: Mutex::new(prefs.post_processing_enabled),
            hotkey_option: Mutex::new(prefs.hotkey_option),
            selected_input_device_uid: Mutex::new(prefs.selected_input_device_uid),
            transcription_history: Mutex::new(Vec::new()),
            api_servers: Mutex::new(prefs.api_servers),
        }
    }
}

impl AppState {
    /// Save current preferences to disk.
    pub fn save_preferences(&self) {
        let prefs = Preferences {
            selected_model_id: self.selected_model_id.lock().unwrap().clone(),
            selected_language: self.selected_language.lock().unwrap().clone(),
            post_processing_enabled: *self.post_processing_enabled.lock().unwrap(),
            hotkey_option: self.hotkey_option.lock().unwrap().clone(),
            selected_input_device_uid: self.selected_input_device_uid.lock().unwrap().clone(),
            api_servers: self.api_servers.lock().unwrap().clone(),
        };
        prefs.save();
    }

    pub fn enqueue(&self, path: PathBuf) -> usize {
        let mut queue = self.transcription_queue.lock().unwrap();
        queue.push(path);
        queue.len()
    }

    pub fn dequeue(&self) -> Option<PathBuf> {
        let mut queue = self.transcription_queue.lock().unwrap();
        if queue.is_empty() {
            None
        } else {
            Some(queue.remove(0))
        }
    }

    pub fn queue_count(&self) -> usize {
        self.transcription_queue.lock().unwrap().len()
    }

    pub fn add_history(&self, text: String) {
        let mut history = self.transcription_history.lock().unwrap();
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
