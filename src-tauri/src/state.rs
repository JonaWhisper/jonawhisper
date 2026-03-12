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
    /// Providers auto-detected from other tools (ephemeral, not saved to prefs).
    pub detected_providers: Mutex<Vec<Provider>>,
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
    let _ = conn.execute("ALTER TABLE history ADD COLUMN raw_text TEXT NOT NULL DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE history ADD COLUMN word_scores TEXT NOT NULL DEFAULT ''", []);

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
            detected_providers: Mutex::new(vec![]),
        }
    }
}

impl AppState {
    /// Run all registered credential detectors and populate `detected_providers`.
    pub fn run_detection(&self) {
        let results = jona_provider::detect_all();
        let mut detected = Vec::new();
        for (cred, detector_id) in results {
            let id = format!("auto-{}-{}", detector_id, cred.kind);
            let preset_name = jona_provider::preset(cred.kind)
                .map(|p| p.display_name)
                .unwrap_or(cred.kind);
            detected.push(Provider {
                id,
                name: format!("{} ({})", preset_name, cred.source_label),
                kind: cred.kind.to_string(),
                url: cred.url,
                api_key: cred.api_key,
                allow_insecure: false,
                cached_models: vec![],
                supports_asr: jona_provider::preset(cred.kind).map(|p| p.supports_asr).unwrap_or(false),
                supports_llm: jona_provider::preset(cred.kind).map(|p| p.supports_llm).unwrap_or(true),
                api_format: None,
                extra: cred.extra,
                enabled: false,
                source: Some(detector_id.to_string()),
            });
        }
        log::info!("Auto-detection: {} provider(s) found", detected.len());
        *self.detected_providers.lock().unwrap() = detected;
    }

    /// Find a provider by ID across both manual and detected providers.
    pub fn find_provider(&self, id: &str) -> Option<Provider> {
        let s = self.settings.lock().unwrap();
        if let Some(p) = s.providers.iter().find(|p| p.id == id) {
            return Some(p.clone());
        }
        drop(s);
        let detected = self.detected_providers.lock().unwrap();
        detected.iter().find(|p| p.id == id).cloned()
    }

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
            "INSERT INTO history (timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed, punctuation_model_id, spellcheck, disfluency_removal, itn, raw_text, word_scores) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![timestamp, entry.text, entry.model_id, entry.language, entry.cleanup_model_id, entry.hallucination_filter, entry.vad_trimmed, entry.punctuation_model_id, entry.spellcheck, entry.disfluency_removal, entry.itn, entry.raw_text, entry.word_scores],
        ) {
            log::error!("Failed to insert history entry: {}", e);
        }
    }

    pub fn get_history(&self, query: &str, limit: u32, cursor: Option<u64>) -> Result<Vec<HistoryEntry>, rusqlite::Error> {
        let db = self.history_db.lock().unwrap();
        const COLS: &str = "timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed, punctuation_model_id, spellcheck, disfluency_removal, itn, raw_text, word_scores";
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
        let mut stmt = db.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let entries = stmt.query_map(params_refs.as_slice(), |row| {
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
                raw_text: row.get(11)?,
                word_scores: row.get(12)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(entries)
    }

    pub fn history_count(&self, query: &str) -> Result<u32, rusqlite::Error> {
        let db = self.history_db.lock().unwrap();
        if query.is_empty() {
            db.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
        } else {
            let pattern = format!("%{}%", query);
            db.query_row("SELECT COUNT(*) FROM history WHERE text LIKE ?1", [&pattern], |row| row.get(0))
        }
    }

    pub fn delete_history_entry(&self, timestamp: u64) -> Result<(), rusqlite::Error> {
        let db = self.history_db.lock().unwrap();
        db.execute("DELETE FROM history WHERE timestamp = ?1", [timestamp])?;
        Ok(())
    }

    pub fn delete_history_day(&self, day_timestamp: u64) -> Result<(), rusqlite::Error> {
        let db = self.history_db.lock().unwrap();
        let day_end = day_timestamp + 86400;
        db.execute(
            "DELETE FROM history WHERE timestamp >= ?1 AND timestamp < ?2",
            [day_timestamp, day_end],
        )?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<(), rusqlite::Error> {
        let db = self.history_db.lock().unwrap();
        db.execute("DELETE FROM history", [])?;
        Ok(())
    }
}

#[cfg(test)]
impl AppState {
    /// Create an AppState with in-memory SQLite for testing.
    fn test_instance() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                 timestamp INTEGER NOT NULL,
                 text TEXT NOT NULL,
                 model_id TEXT NOT NULL DEFAULT '',
                 language TEXT NOT NULL DEFAULT '',
                 cleanup_model_id TEXT NOT NULL DEFAULT '',
                 hallucination_filter INTEGER NOT NULL DEFAULT 0,
                 vad_trimmed INTEGER NOT NULL DEFAULT 0,
                 punctuation_model_id TEXT NOT NULL DEFAULT '',
                 spellcheck INTEGER NOT NULL DEFAULT 0,
                 disfluency_removal INTEGER NOT NULL DEFAULT 0,
                 itn INTEGER NOT NULL DEFAULT 0,
                 raw_text TEXT NOT NULL DEFAULT '',
                 word_scores TEXT NOT NULL DEFAULT ''
             );
             CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC);"
        ).unwrap();
        Self {
            runtime: Mutex::new(RuntimeState::default()),
            download: Arc::new(Mutex::new(DownloadState::default())),
            settings: Mutex::new(Preferences::default()),
            history_db: Mutex::new(conn),
            tray_menu: Mutex::new(None),
            contexts: ContextMap::new(),
            audio_flags: AudioFlags::default(),
            detected_providers: Mutex::new(vec![]),
        }
    }

    /// Insert a history entry with a specific timestamp (for test control).
    fn add_history_at(&self, timestamp: u64, entry: HistoryEntry) {
        let db = self.history_db.lock().unwrap();
        let _ = db.execute(
            "INSERT INTO history (timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed, punctuation_model_id, spellcheck, disfluency_removal, itn) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![timestamp, entry.text, entry.model_id, entry.language, entry.cleanup_model_id, entry.hallucination_filter, entry.vad_trimmed, entry.punctuation_model_id, entry.spellcheck, entry.disfluency_removal, entry.itn],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str, model: &str, lang: &str) -> HistoryEntry {
        HistoryEntry {
            text: text.to_string(),
            timestamp: 0, // overridden by add_history_at
            model_id: model.to_string(),
            language: lang.to_string(),
            ..Default::default()
        }
    }

    // =========================================================================
    // History persistence — user's transcriptions must survive across sessions
    // =========================================================================

    #[test]
    fn transcription_saved_and_retrieved() {
        let state = AppState::test_instance();
        state.add_history_at(1000, entry("Bonjour le monde", "whisper:large-v3", "fr"));

        let results = state.get_history("", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Bonjour le monde");
        assert_eq!(results[0].model_id, "whisper:large-v3");
        assert_eq!(results[0].language, "fr");
    }

    #[test]
    fn history_ordered_most_recent_first() {
        let state = AppState::test_instance();
        state.add_history_at(100, entry("First", "whisper:tiny", "en"));
        state.add_history_at(300, entry("Third", "whisper:tiny", "en"));
        state.add_history_at(200, entry("Second", "whisper:tiny", "en"));

        let results = state.get_history("", 10, None).unwrap();
        assert_eq!(results[0].text, "Third");
        assert_eq!(results[1].text, "Second");
        assert_eq!(results[2].text, "First");
    }

    #[test]
    fn history_count_reflects_total() {
        let state = AppState::test_instance();
        assert_eq!(state.history_count("").unwrap(), 0);

        state.add_history_at(100, entry("One", "", ""));
        state.add_history_at(200, entry("Two", "", ""));
        state.add_history_at(300, entry("Three", "", ""));
        assert_eq!(state.history_count("").unwrap(), 3);
    }

    #[test]
    fn history_limit_caps_results() {
        let state = AppState::test_instance();
        for i in 0..20 {
            state.add_history_at(i, entry(&format!("Entry {}", i), "", ""));
        }

        let results = state.get_history("", 5, None).unwrap();
        assert_eq!(results.len(), 5);
    }

    // -- Cursor-based pagination (infinite scroll) --

    #[test]
    fn cursor_pagination_returns_older_entries() {
        let state = AppState::test_instance();
        state.add_history_at(100, entry("Old", "", ""));
        state.add_history_at(200, entry("Middle", "", ""));
        state.add_history_at(300, entry("Recent", "", ""));

        // First page: most recent
        let page1 = state.get_history("", 2, None).unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].text, "Recent");
        assert_eq!(page1[1].text, "Middle");

        // Second page: cursor = timestamp of last entry on page 1
        let cursor = page1[1].timestamp;
        let page2 = state.get_history("", 2, Some(cursor)).unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].text, "Old");
    }

    #[test]
    fn cursor_past_all_entries_returns_empty() {
        let state = AppState::test_instance();
        state.add_history_at(100, entry("Only one", "", ""));

        let results = state.get_history("", 10, Some(50)).unwrap();
        assert!(results.is_empty());
    }

    // -- Search --

    #[test]
    fn search_filters_by_text() {
        let state = AppState::test_instance();
        state.add_history_at(100, entry("Bonjour le monde", "", "fr"));
        state.add_history_at(200, entry("Hello world", "", "en"));
        state.add_history_at(300, entry("Bonsoir tout le monde", "", "fr"));

        let results = state.get_history("monde", 10, None).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.text.contains("monde")));
    }

    #[test]
    fn search_case_insensitive() {
        let state = AppState::test_instance();
        state.add_history_at(100, entry("BONJOUR", "", ""));

        // SQLite LIKE is case-insensitive for ASCII
        let results = state.get_history("bonjour", 10, None).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_count_matches_results() {
        let state = AppState::test_instance();
        state.add_history_at(100, entry("Bonjour", "", ""));
        state.add_history_at(200, entry("Hello", "", ""));
        state.add_history_at(300, entry("Bon appétit", "", ""));

        assert_eq!(state.history_count("Bon").unwrap(), 2);
        assert_eq!(state.history_count("Hello").unwrap(), 1);
        assert_eq!(state.history_count("xyz").unwrap(), 0);
    }

    #[test]
    fn search_with_cursor() {
        let state = AppState::test_instance();
        state.add_history_at(100, entry("Bonjour A", "", ""));
        state.add_history_at(200, entry("Hello", "", ""));
        state.add_history_at(300, entry("Bonjour B", "", ""));

        let page1 = state.get_history("Bonjour", 1, None).unwrap();
        assert_eq!(page1.len(), 1);
        assert_eq!(page1[0].text, "Bonjour B");

        let page2 = state.get_history("Bonjour", 1, Some(page1[0].timestamp)).unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].text, "Bonjour A");
    }

    // -- Deletion --

    #[test]
    fn delete_single_entry() {
        let state = AppState::test_instance();
        state.add_history_at(100, entry("Keep me", "", ""));
        state.add_history_at(200, entry("Delete me", "", ""));

        state.delete_history_entry(200).unwrap();
        let results = state.get_history("", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Keep me");
    }

    #[test]
    fn delete_day_removes_24h_window() {
        let state = AppState::test_instance();
        let day_start: u64 = 1700000000; // some day
        state.add_history_at(day_start + 100, entry("Morning", "", ""));
        state.add_history_at(day_start + 50000, entry("Afternoon", "", ""));
        state.add_history_at(day_start + 86400 + 100, entry("Next day", "", ""));
        state.add_history_at(day_start - 100, entry("Previous day", "", ""));

        state.delete_history_day(day_start).unwrap();

        let results = state.get_history("", 10, None).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|e| e.text == "Next day"));
        assert!(results.iter().any(|e| e.text == "Previous day"));
    }

    #[test]
    fn clear_history_removes_all() {
        let state = AppState::test_instance();
        for i in 0..10 {
            state.add_history_at(i * 100, entry(&format!("Entry {}", i), "", ""));
        }
        assert_eq!(state.history_count("").unwrap(), 10);

        state.clear_history().unwrap();
        assert_eq!(state.history_count("").unwrap(), 0);
        assert!(state.get_history("", 10, None).unwrap().is_empty());
    }

    // -- Metadata preservation --

    #[test]
    fn history_preserves_pipeline_metadata() {
        let state = AppState::test_instance();
        state.add_history_at(100, HistoryEntry {
            text: "Test".to_string(),
            timestamp: 0,
            model_id: "whisper:large-v3".to_string(),
            language: "fr".to_string(),
            cleanup_model_id: "correction:gec-t5-small".to_string(),
            hallucination_filter: true,
            vad_trimmed: true,
            punctuation_model_id: "punctuation:pcs".to_string(),
            spellcheck: true,
            disfluency_removal: true,
            itn: true,
            ..Default::default()
        });

        let results = state.get_history("", 10, None).unwrap();
        let e = &results[0];
        assert_eq!(e.cleanup_model_id, "correction:gec-t5-small");
        assert!(e.hallucination_filter);
        assert!(e.vad_trimmed);
        assert_eq!(e.punctuation_model_id, "punctuation:pcs");
        assert!(e.spellcheck);
        assert!(e.disfluency_removal);
        assert!(e.itn);
    }

    // -- Queue (recording pipeline) --

    #[test]
    fn recording_queue_fifo_order() {
        let state = AppState::test_instance();
        state.enqueue(std::path::PathBuf::from("/tmp/a.wav"));
        state.enqueue(std::path::PathBuf::from("/tmp/b.wav"));
        state.enqueue(std::path::PathBuf::from("/tmp/c.wav"));

        assert_eq!(state.queue_count(), 3);
        assert_eq!(state.dequeue().unwrap(), std::path::PathBuf::from("/tmp/a.wav"));
        assert_eq!(state.dequeue().unwrap(), std::path::PathBuf::from("/tmp/b.wav"));
        assert_eq!(state.queue_count(), 1);
    }

    #[test]
    fn empty_queue_returns_none() {
        let state = AppState::test_instance();
        assert_eq!(state.queue_count(), 0);
        assert!(state.dequeue().is_none());
    }

    // -- Frontend JSON --

    #[test]
    fn frontend_json_reflects_runtime_state() {
        let state = AppState::test_instance();
        {
            let mut rt = state.runtime.lock().unwrap();
            rt.is_recording = true;
            rt.is_transcribing = false;
            rt.queue.push_back(std::path::PathBuf::from("/tmp/test.wav"));
        }

        let json = state.to_frontend_json();
        assert_eq!(json["is_recording"], true);
        assert_eq!(json["is_transcribing"], false);
        assert_eq!(json["queue_count"], 1);
    }
}
