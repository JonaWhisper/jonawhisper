use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "WhisperDictate";
const PREFS_FILE: &str = "preferences.json";
const HISTORY_DB: &str = "history.db";
const HISTORY_JSON_LEGACY: &str = "history.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub text: String,
    pub timestamp: u64,
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub language: String,
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
    pub history_db: Mutex<Connection>,
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

fn open_history_db() -> Connection {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join(HISTORY_DB);
    let conn = Connection::open(&db_path).expect("Failed to open history database");

    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         CREATE TABLE IF NOT EXISTS history (
             timestamp INTEGER NOT NULL,
             text TEXT NOT NULL,
             model_id TEXT NOT NULL DEFAULT '',
             language TEXT NOT NULL DEFAULT ''
         );
         CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC);"
    ).expect("Failed to initialize history schema");

    // Migrate legacy history.json if it exists
    let json_path = dir.join(HISTORY_JSON_LEGACY);
    if json_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&json_path) {
            if let Ok(entries) = serde_json::from_str::<Vec<HistoryEntry>>(&data) {
                let tx = conn.unchecked_transaction().expect("Failed to start migration tx");
                for entry in &entries {
                    let _ = tx.execute(
                        "INSERT OR IGNORE INTO history (timestamp, text, model_id, language) VALUES (?1, ?2, ?3, ?4)",
                        rusqlite::params![entry.timestamp, entry.text, entry.model_id, entry.language],
                    );
                }
                let _ = tx.commit();
                log::info!("Migrated {} history entries from JSON to SQLite", entries.len());
            }
        }
        let _ = std::fs::remove_file(&json_path);
    }

    conn
}

impl Default for AppState {
    fn default() -> Self {
        let prefs = Preferences::load();
        Self {
            runtime: Mutex::new(RuntimeState::default()),
            download: Mutex::new(DownloadState::default()),
            settings: Mutex::new(prefs),
            history_db: Mutex::new(open_history_db()),
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

    pub fn add_history(&self, text: String, model_id: String, language: String) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let db = self.history_db.lock().unwrap();
        if let Err(e) = db.execute(
            "INSERT INTO history (timestamp, text, model_id, language) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![timestamp, text, model_id, language],
        ) {
            log::error!("Failed to insert history entry: {}", e);
        }
    }

    pub fn get_history(&self) -> Vec<HistoryEntry> {
        let db = self.history_db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT timestamp, text, model_id, language FROM history ORDER BY timestamp DESC"
        ).unwrap();
        stmt.query_map([], |row| {
            Ok(HistoryEntry {
                timestamp: row.get(0)?,
                text: row.get(1)?,
                model_id: row.get(2)?,
                language: row.get(3)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn search_history(&self, query: &str) -> Vec<HistoryEntry> {
        let db = self.history_db.lock().unwrap();
        let pattern = format!("%{}%", query);
        let mut stmt = db.prepare(
            "SELECT timestamp, text, model_id, language FROM history WHERE text LIKE ?1 ORDER BY timestamp DESC"
        ).unwrap();
        stmt.query_map([&pattern], |row| {
            Ok(HistoryEntry {
                timestamp: row.get(0)?,
                text: row.get(1)?,
                model_id: row.get(2)?,
                language: row.get(3)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn delete_history_entry(&self, timestamp: u64) {
        let db = self.history_db.lock().unwrap();
        let _ = db.execute("DELETE FROM history WHERE timestamp = ?1", [timestamp]);
    }

    pub fn delete_history_day(&self, day_timestamp: u64) {
        let db = self.history_db.lock().unwrap();
        let day_end = day_timestamp + 86400;
        let _ = db.execute(
            "DELETE FROM history WHERE timestamp >= ?1 AND timestamp < ?2",
            [day_timestamp, day_end],
        );
    }

    pub fn clear_history(&self) {
        let db = self.history_db.lock().unwrap();
        let _ = db.execute("DELETE FROM history", []);
    }
}
