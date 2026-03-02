use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::path::PathBuf;

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
macro_rules! context_group {
    ($name:ident { $($field:ident : $ctx:ty),* $(,)? }) => {
        pub struct $name { $(pub $field: ContextSlot<$ctx>,)* }
        impl $name {
            pub fn new() -> Self { Self { $($field: ContextSlot::empty(),)* } }
            pub fn invalidate_all(&self) { $(self.$field.invalidate();)* }
        }
    };
}

context_group!(AsrContexts {
    whisper: crate::engines::whisper::WhisperCtx,
    canary: crate::canary_asr::CanaryContext,
    parakeet: crate::parakeet_asr::ParakeetContext,
    qwen: crate::qwen_asr::QwenContext,
});

context_group!(CleanupContexts {
    llm: crate::llm_local::LlmContext,
    bert: crate::bert_punctuation::BertContext,
    candle_punct: crate::candle_punctuation::CandlePunctContext,
    pcs: crate::pcs_punctuation::PcsContext,
    t5: crate::t5_correction::T5Context,
});

/// All inference contexts, grouped by ASR and cleanup.
pub struct InferenceContexts {
    pub asr: AsrContexts,
    pub cleanup: CleanupContexts,
}

impl InferenceContexts {
    pub fn new() -> Self {
        Self {
            asr: AsrContexts::new(),
            cleanup: CleanupContexts::new(),
        }
    }
}

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
    #[serde(default)]
    pub cleanup_model_id: String,
    #[serde(default)]
    pub hallucination_filter: bool,
    #[serde(default)]
    pub vad_trimmed: bool,
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
    /// Cached inference contexts (ASR + cleanup models). Each slot is lazy-loaded and
    /// invalidated when the corresponding model selection changes.
    pub inference: InferenceContexts,
}

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

impl Provider {
    /// Resolved base URL: preset URL for known providers, stored URL for Custom.
    pub fn base_url(&self) -> &str {
        self.kind.base_url().unwrap_or_else(|| self.url.trim_end_matches('/'))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub url: String,
    pub api_key: String,
    #[serde(default)]
    pub cached_models: Vec<String>,
}

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
    #[serde(default = "default_recording_mode")]
    pub recording_mode: String,
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
    #[serde(default = "default_gpu_mode")]
    pub gpu_mode: String,
    #[serde(default = "default_llm_max_tokens")]
    pub llm_max_tokens: u32,
    #[serde(default)]
    pub audio_ducking_enabled: bool,
    #[serde(default = "default_ducking_level")]
    pub audio_ducking_level: f32,
    #[serde(default = "default_true")]
    pub vad_enabled: bool,
}

pub fn default_model_id() -> String { "whisper:large-v3-turbo-q8".to_string() }
fn default_language() -> String { "auto".to_string() }
fn default_true() -> bool { true }
fn default_hotkey() -> String { "right_command".to_string() }
fn default_auto() -> String { "auto".to_string() }
fn default_cancel_shortcut() -> String { "escape".to_string() }
fn default_asr_cloud_model() -> String { "whisper-1".to_string() }
fn default_recording_mode() -> String { "push_to_talk".to_string() }
fn default_gpu_mode() -> String { "auto".to_string() }
fn default_llm_max_tokens() -> u32 { 4096 }
fn default_ducking_level() -> f32 { 0.8 }

/// Config directory: ~/Library/Application Support/WhisperDictate/ (macOS)
/// or %APPDATA%/WhisperDictate/ (Windows).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join(APP_DIR_NAME)
}

/// Model storage: ~/Library/Application Support/WhisperDictate/models/
pub fn models_dir() -> PathBuf {
    config_dir().join("models")
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

        let mut raw: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
        let mut prefs: Preferences = serde_json::from_value(raw.clone()).unwrap_or_default();

        if crate::migrations::run(&mut raw, &mut prefs) {
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

    // Additive migrations
    let _ = conn.execute("ALTER TABLE history ADD COLUMN cleanup_model_id TEXT NOT NULL DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE history ADD COLUMN hallucination_filter INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE history ADD COLUMN vad_trimmed INTEGER NOT NULL DEFAULT 0", []);

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
            inference: InferenceContexts::new(),
        }
    }
}

impl AppState {
    /// Save current preferences to disk.
    pub fn save_preferences(&self) {
        let settings = self.settings.lock().unwrap();
        log::info!("save_preferences: hallucination_filter={}", settings.hallucination_filter_enabled);
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

    pub fn add_history(&self, text: String, model_id: String, language: String, cleanup_model_id: String, hallucination_filter: bool, vad_trimmed: bool) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let db = self.history_db.lock().unwrap();
        if let Err(e) = db.execute(
            "INSERT INTO history (timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed],
        ) {
            log::error!("Failed to insert history entry: {}", e);
        }
    }

    pub fn get_history(&self, query: &str, limit: u32, offset: u32) -> Vec<HistoryEntry> {
        let db = self.history_db.lock().unwrap();
        let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = if query.is_empty() {
            (
                "SELECT timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed FROM history ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
                vec![Box::new(limit), Box::new(offset)],
            )
        } else {
            let pattern = format!("%{}%", query);
            (
                "SELECT timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed FROM history WHERE text LIKE ?1 ORDER BY timestamp DESC LIMIT ?2 OFFSET ?3",
                vec![Box::new(pattern), Box::new(limit), Box::new(offset)],
            )
        };
        let mut stmt = db.prepare(sql).unwrap();
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        stmt.query_map(params_refs.as_slice(), |row| {
            Ok(HistoryEntry {
                timestamp: row.get(0)?,
                text: row.get(1)?,
                model_id: row.get(2)?,
                language: row.get(3)?,
                cleanup_model_id: row.get(4)?,
                hallucination_filter: row.get(5)?,
                vad_trimmed: row.get(6)?,
            })
        }).unwrap().filter_map(|r| r.ok()).collect()
    }

    pub fn history_count(&self, query: &str) -> u32 {
        let db = self.history_db.lock().unwrap();
        if query.is_empty() {
            db.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0)).unwrap_or(0)
        } else {
            let pattern = format!("%{}%", query);
            db.query_row("SELECT COUNT(*) FROM history WHERE text LIKE ?1", [&pattern], |row| row.get(0)).unwrap_or(0)
        }
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
