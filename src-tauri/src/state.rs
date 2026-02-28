use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
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

// -- Main AppState --

pub struct AppState {
    pub runtime: Mutex<RuntimeState>,
    pub download: Mutex<DownloadState>,
    pub settings: Mutex<Preferences>,
    pub history_db: Mutex<Connection>,
    pub tray_menu: Mutex<Option<crate::tray::TrayMenuState>>,
    /// Cached WhisperContext: (model_id, gpu_mode, context). Invalidated when model or GPU mode changes.
    pub whisper_context: Mutex<Option<(String, String, whisper_rs::WhisperContext)>>,
    /// Cached LLM context for local inference. Invalidated when llm_local_model_id changes.
    pub llm_context: Mutex<Option<crate::llm_local::LlmContext>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Custom,
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub url: String,
    pub api_key: String,
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
    pub providers: Vec<Provider>,
    #[serde(default = "default_auto")]
    pub app_locale: String,
    #[serde(default = "default_true")]
    pub hallucination_filter_enabled: bool,
    #[serde(default = "default_cancel_shortcut")]
    pub cancel_shortcut: String,
    #[serde(default = "default_recording_mode")]
    pub recording_mode: String,
    #[serde(default)]
    pub llm_enabled: bool,
    #[serde(default)]
    pub llm_provider_id: String,
    #[serde(default)]
    pub llm_model: String,
    #[serde(default = "default_llm_source")]
    pub llm_source: String,
    #[serde(default)]
    pub llm_local_model_id: String,
    #[serde(default)]
    pub asr_provider_id: String,
    #[serde(default = "default_asr_cloud_model")]
    pub asr_cloud_model: String,
    #[serde(default = "default_gpu_mode")]
    pub gpu_mode: String,
    #[serde(default = "default_llm_max_tokens")]
    pub llm_max_tokens: u32,
}

fn default_model_id() -> String { "whisper:large-v3-turbo-q8".to_string() }
fn default_language() -> String { "auto".to_string() }
fn default_true() -> bool { true }
fn default_hotkey() -> String { "right_command".to_string() }
fn default_auto() -> String { "auto".to_string() }
fn default_cancel_shortcut() -> String { "escape".to_string() }
fn default_asr_cloud_model() -> String { "whisper-1".to_string() }
fn default_recording_mode() -> String { "push_to_talk".to_string() }
fn default_gpu_mode() -> String { "auto".to_string() }
fn default_llm_source() -> String { "cloud".to_string() }
fn default_llm_max_tokens() -> u32 { 256 }

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
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => return Self::default(),
        };

        // Try parsing as new format first
        let mut prefs: Preferences = serde_json::from_str(&data).unwrap_or_default();
        let mut needs_save = false;

        // Detect old format and migrate
        if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&data) {
            let has_old_api_servers = raw.get("api_servers").is_some();
            let has_old_llm_config = raw.get("llm_config").is_some();

            if has_old_api_servers || has_old_llm_config {
                log::info!("Migrating preferences from old format to unified providers");
                needs_save = true;

                // 1. Convert api_servers → providers
                if let Some(servers) = raw.get("api_servers").and_then(|v| v.as_array()) {
                    for server in servers {
                        if let (Some(id), Some(name), Some(url), Some(model)) = (
                            server.get("id").and_then(|v| v.as_str()),
                            server.get("name").and_then(|v| v.as_str()),
                            server.get("url").and_then(|v| v.as_str()),
                            server.get("model").and_then(|v| v.as_str()),
                        ) {
                            let api_key = server.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
                            prefs.providers.push(Provider {
                                id: id.to_string(),
                                name: name.to_string(),
                                kind: ProviderKind::Custom,
                                url: url.to_string(),
                                api_key: api_key.to_string(),
                            });
                            // Migrate ASR model to settings
                            if !model.is_empty() && prefs.asr_provider_id.is_empty() {
                                prefs.asr_provider_id = id.to_string();
                                prefs.asr_cloud_model = model.to_string();
                            }
                        }
                    }
                }

                // 2. Convert llm_config → provider + settings
                if let Some(llm) = raw.get("llm_config") {
                    let enabled = llm.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
                    let provider_str = llm.get("provider").and_then(|v| v.as_str()).unwrap_or("openai");
                    let api_url = llm.get("api_url").and_then(|v| v.as_str()).unwrap_or("");
                    let api_key = llm.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
                    let model = llm.get("model").and_then(|v| v.as_str()).unwrap_or("");

                    prefs.llm_enabled = enabled;
                    prefs.llm_model = model.to_string();

                    if !api_url.is_empty() {
                        // Check if an existing provider has the same url+key
                        let existing = prefs.providers.iter().find(|p|
                            p.url == api_url && p.api_key == api_key
                        );
                        if let Some(p) = existing {
                            prefs.llm_provider_id = p.id.clone();
                        } else {
                            let kind = if provider_str == "anthropic" {
                                ProviderKind::Anthropic
                            } else {
                                ProviderKind::OpenAI
                            };
                            let id = format!("provider-{}", provider_str);
                            prefs.providers.push(Provider {
                                id: id.clone(),
                                name: kind.display_name().to_string(),
                                kind,
                                url: api_url.to_string(),
                                api_key: api_key.to_string(),
                            });
                            prefs.llm_provider_id = id;
                        }
                    }
                }
            }

            // Migrate providers that still have asr_model in JSON (old unified format)
            if let Some(providers_json) = raw.get("providers").and_then(|v| v.as_array()) {
                for pj in providers_json {
                    if let Some(asr_model) = pj.get("asr_model").and_then(|v| v.as_str()) {
                        if !asr_model.is_empty() && prefs.asr_provider_id.is_empty() {
                            if let Some(pid) = pj.get("id").and_then(|v| v.as_str()) {
                                log::info!("Migrating asr_model from provider {} to settings", pid);
                                prefs.asr_provider_id = pid.to_string();
                                prefs.asr_cloud_model = asr_model.to_string();
                                needs_save = true;
                            }
                        }
                    }
                }
            }

            // Reset selected_model_id if it was pointing to old openai-api: pseudo-model
            if prefs.selected_model_id.starts_with("openai-api:") {
                log::info!("Resetting selected_model_id from old openai-api: format");
                prefs.selected_model_id = default_model_id();
                needs_save = true;
            }

            // Migrate llm_local_model_id from old "llm-local:" prefix to "llama:"
            if prefs.llm_local_model_id.starts_with("llm-local:") {
                let new_id = prefs.llm_local_model_id.replacen("llm-local:", "llama:", 1);
                log::info!("Migrating llm_local_model_id: {} → {}", prefs.llm_local_model_id, new_id);
                prefs.llm_local_model_id = new_id;
                needs_save = true;
            }

            // Migrate: if llm_enabled with a cloud provider but no llm_source set, default to cloud
            if prefs.llm_enabled && !prefs.llm_provider_id.is_empty() && prefs.llm_source == "cloud" {
                // Already correct default, no migration needed
            }
        }

        if needs_save {
            prefs.save();
            log::info!("Migration complete: {} providers", prefs.providers.len());
        }

        prefs
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
            tray_menu: Mutex::new(None),
            whisper_context: Mutex::new(None),
            llm_context: Mutex::new(None),
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
        let active_downloads: serde_json::Map<String, serde_json::Value> = dl.active.iter()
            .map(|(id, d)| (id.clone(), serde_json::json!(d.progress)))
            .collect();
        serde_json::json!({
            "is_recording": rt.is_recording,
            "is_transcribing": rt.is_transcribing,
            "queue_count": rt.queue.len(),
            "active_downloads": active_downloads,
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
