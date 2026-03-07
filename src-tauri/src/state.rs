// Re-export everything from jona-types for backward compatibility
pub use jona_types::*;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

// -- Main AppState --

pub struct AppState {
    pub runtime: Mutex<RuntimeState>,
    pub download: Arc<Mutex<DownloadState>>,
    pub settings: Mutex<Preferences>,
    pub history_db: Mutex<Connection>,
    pub tray_menu: Mutex<Option<crate::ui::tray::TrayMenuState>>,
    /// Dynamic context map for all engine inference contexts (ASR + cleanup).
    /// Replaces the old typed `InferenceContexts` with type-erased storage.
    pub contexts: ContextMap,
    /// Lock-free flags for spectrum emitter hot path.
    pub audio_flags: AudioFlags,
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
    let _ = conn.execute("ALTER TABLE history ADD COLUMN punctuation_model_id TEXT NOT NULL DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE history ADD COLUMN spellcheck INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE history ADD COLUMN disfluency_removal INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE history ADD COLUMN itn INTEGER NOT NULL DEFAULT 0", []);

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

/// Load preferences with migration support.
fn load_preferences() -> Preferences {
    // Rename WhisperDictate/ → JonaWhisper/ before reading config
    crate::migrations::migrate_data_directory();

    let path = prefs_path();
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return Preferences::default(),
    };

    let mut raw: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
    let mut prefs: Preferences = serde_json::from_value(raw.clone()).unwrap_or_default();

    if crate::migrations::run(&mut raw, &mut prefs) {
        prefs.save();
        log::info!("Migration complete: {} providers", prefs.providers.len());
    }

    // Populate API keys from OS keychain (keys are no longer stored in JSON)
    load_api_keys_from_keyring(&mut prefs.providers);

    prefs
}

impl Default for AppState {
    fn default() -> Self {
        let prefs = load_preferences();
        Self {
            runtime: Mutex::new(RuntimeState::default()),
            download: Arc::new(Mutex::new(DownloadState::default())),
            settings: Mutex::new(prefs),
            history_db: Mutex::new(open_history_db()),
            tray_menu: Mutex::new(None),
            contexts: ContextMap::new(),
            audio_flags: AudioFlags::default(),
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

    pub fn enqueue(&self, path: std::path::PathBuf) -> usize {
        let mut rt = self.runtime.lock().unwrap();
        rt.queue.push_back(path);
        rt.queue.len()
    }

    pub fn dequeue(&self) -> Option<std::path::PathBuf> {
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

    pub fn add_history(&self, entry: HistoryEntry) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let db = self.history_db.lock().unwrap();
        if let Err(e) = db.execute(
            "INSERT INTO history (timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed, punctuation_model_id, spellcheck, disfluency_removal, itn) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![timestamp, entry.text, entry.model_id, entry.language, entry.cleanup_model_id, entry.hallucination_filter, entry.vad_trimmed, entry.punctuation_model_id, entry.spellcheck, entry.disfluency_removal, entry.itn],
        ) {
            log::error!("Failed to insert history entry: {}", e);
        }
    }

    pub fn get_history(&self, query: &str, limit: u32, cursor: Option<u64>) -> Vec<HistoryEntry> {
        let db = self.history_db.lock().unwrap();
        const COLS: &str = "timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed, punctuation_model_id, spellcheck, disfluency_removal, itn";
        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match (query.is_empty(), cursor) {
            (true, None) => (
                format!("SELECT {COLS} FROM history ORDER BY timestamp DESC LIMIT ?1"),
                vec![Box::new(limit)],
            ),
            (true, Some(c)) => (
                format!("SELECT {COLS} FROM history WHERE timestamp < ?1 ORDER BY timestamp DESC LIMIT ?2"),
                vec![Box::new(c), Box::new(limit)],
            ),
            (false, None) => {
                let pattern = format!("%{}%", query);
                (
                    format!("SELECT {COLS} FROM history WHERE text LIKE ?1 ORDER BY timestamp DESC LIMIT ?2"),
                    vec![Box::new(pattern), Box::new(limit)],
                )
            }
            (false, Some(c)) => {
                let pattern = format!("%{}%", query);
                (
                    format!("SELECT {COLS} FROM history WHERE text LIKE ?1 AND timestamp < ?2 ORDER BY timestamp DESC LIMIT ?3"),
                    vec![Box::new(pattern), Box::new(c), Box::new(limit)],
                )
            }
        };
        let mut stmt = db.prepare(&sql).unwrap();
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
                punctuation_model_id: row.get(7)?,
                spellcheck: row.get(8)?,
                disfluency_removal: row.get(9)?,
                itn: row.get(10)?,
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
