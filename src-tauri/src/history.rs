use jona_types::{HistoryEntry, HISTORY_DB, HISTORY_JSON_LEGACY, config_dir};
use rusqlite::Connection;
use std::sync::Mutex;

/// Sole owner of the `history` table: no SQL against it exists outside this module.
pub struct HistoryStore {
    db: Mutex<Connection>,
}

/// Shared by production and tests: a test table built by hand drifts from this one,
/// and every reader indexes columns positionally.
fn init_history_schema(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
             timestamp INTEGER NOT NULL,
             text TEXT NOT NULL,
             model_id TEXT NOT NULL DEFAULT '',
             language TEXT NOT NULL DEFAULT ''
         );
         CREATE INDEX IF NOT EXISTS idx_history_timestamp ON history(timestamp DESC);",
    )
    .expect("Failed to initialize history schema");

    // Additive migrations: each is a no-op once the column exists.
    for stmt in [
        "ALTER TABLE history ADD COLUMN cleanup_model_id TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE history ADD COLUMN hallucination_filter INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE history ADD COLUMN vad_trimmed INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE history ADD COLUMN punctuation_model_id TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE history ADD COLUMN spellcheck INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE history ADD COLUMN disfluency_removal INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE history ADD COLUMN itn INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE history ADD COLUMN raw_text TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE history ADD COLUMN word_scores TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE history ADD COLUMN rating INTEGER NOT NULL DEFAULT 0",
    ] {
        let _ = conn.execute(stmt, []);
    }
}

fn open_history_db() -> Connection {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join(HISTORY_DB);
    let conn = Connection::open(&db_path).expect("Failed to open history database");

    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
        .expect("Failed to set history pragmas");
    init_history_schema(&conn);

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

impl HistoryStore {
    pub fn open() -> Self {
        Self { db: Mutex::new(open_history_db()) }
    }

    /// Insert a history entry with the current timestamp.
    /// Note: `entry.timestamp` is ignored — a fresh timestamp is always generated.
    pub fn add(&self, entry: HistoryEntry) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let db = self.db.lock().unwrap();
        if let Err(e) = db.execute(
            "INSERT INTO history (timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed, punctuation_model_id, spellcheck, disfluency_removal, itn, raw_text, word_scores) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![timestamp, entry.text, entry.model_id, entry.language, entry.cleanup_model_id, entry.hallucination_filter, entry.vad_trimmed, entry.punctuation_model_id, entry.spellcheck, entry.disfluency_removal, entry.itn, entry.raw_text, entry.word_scores],
        ) {
            log::error!("Failed to insert history entry: {}", e);
        }
    }

    /// Store the text the user says should have come out. It lands as a final
    /// `manual` pipeline step, so the history renders it like every other stage,
    /// and becomes the entry's text — the corrected version is the right one.
    pub fn set_correction(&self, timestamp: u64, corrected: &str) -> Result<(), rusqlite::Error> {
        let db = self.db.lock().unwrap();
        let (text, raw): (String, String) = db.query_row(
            "SELECT text, raw_text FROM history WHERE timestamp = ?1",
            rusqlite::params![timestamp],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let mut steps: Vec<(String, String)> =
            serde_json::from_str(&raw).unwrap_or_default();
        // Entries recorded before pipeline tracking have no steps to diff against.
        if steps.is_empty() {
            steps.push(("asr".to_string(), text));
        }
        steps.retain(|(name, _)| name != "manual");
        steps.push(("manual".to_string(), corrected.to_string()));

        let updated = serde_json::to_string(&steps).unwrap_or(raw);
        db.execute(
            "UPDATE history SET text = ?1, raw_text = ?2 WHERE timestamp = ?3",
            rusqlite::params![corrected, updated, timestamp],
        )?;
        Ok(())
    }

    /// Record the user's verdict on one dictation: 1 good, -1 bad, 0 clears it.
    pub fn set_rating(&self, timestamp: u64, rating: i8) -> Result<(), rusqlite::Error> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE history SET rating = ?1 WHERE timestamp = ?2",
            rusqlite::params![rating, timestamp],
        )?;
        Ok(())
    }

    pub fn get(&self, query: &str, limit: u32, cursor: Option<u64>) -> Result<Vec<HistoryEntry>, rusqlite::Error> {
        let db = self.db.lock().unwrap();
        const COLS: &str = "timestamp, text, model_id, language, cleanup_model_id, hallucination_filter, vad_trimmed, punctuation_model_id, spellcheck, disfluency_removal, itn, raw_text, word_scores, rating";
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
                let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
                let pattern = format!("%{}%", escaped);
                (
                    format!("SELECT {COLS} FROM history WHERE text LIKE ?1 ESCAPE '\\' ORDER BY timestamp DESC LIMIT ?2"),
                    vec![Box::new(pattern), Box::new(limit)],
                )
            }
            (false, Some(c)) => {
                let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
                let pattern = format!("%{}%", escaped);
                (
                    format!("SELECT {COLS} FROM history WHERE text LIKE ?1 ESCAPE '\\' AND timestamp < ?2 ORDER BY timestamp DESC LIMIT ?3"),
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
                rating: row.get(13)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(entries)
    }

    pub fn count(&self, query: &str) -> Result<u32, rusqlite::Error> {
        let db = self.db.lock().unwrap();
        if query.is_empty() {
            db.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
        } else {
            let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            let pattern = format!("%{}%", escaped);
            db.query_row("SELECT COUNT(*) FROM history WHERE text LIKE ?1 ESCAPE '\\'", [&pattern], |row| row.get(0))
        }
    }

    pub fn delete_entry(&self, timestamp: u64) -> Result<(), rusqlite::Error> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM history WHERE timestamp = ?1", [timestamp])?;
        Ok(())
    }

    pub fn delete_day(&self, day_timestamp: u64) -> Result<(), rusqlite::Error> {
        let db = self.db.lock().unwrap();
        let day_end = day_timestamp + 86400;
        db.execute(
            "DELETE FROM history WHERE timestamp >= ?1 AND timestamp < ?2",
            [day_timestamp, day_end],
        )?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), rusqlite::Error> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM history", [])?;
        Ok(())
    }

    // These two ran under a poisoned-mutex-tolerant lock in settings.rs before the
    // move; uniforming them onto unwrap() would change that behaviour.
    pub fn dominant_language(&self) -> Result<String, rusqlite::Error> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        db.query_row(
            "SELECT language FROM history WHERE language <> '' \
             GROUP BY language ORDER BY COUNT(*) DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
    }

    pub fn texts_for_language(&self, language: &str) -> Result<Vec<String>, rusqlite::Error> {
        let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = db.prepare("SELECT text FROM history WHERE language = ?1")?;
        let texts = stmt
            .query_map([language], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(texts)
    }
}

#[cfg(test)]
impl HistoryStore {
    pub(crate) fn in_memory() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        init_history_schema(&conn);
        Self { db: Mutex::new(conn) }
    }

    /// Insert a history entry with a specific timestamp (for test control).
    fn add_at(&self, timestamp: u64, entry: HistoryEntry) {
        let db = self.db.lock().unwrap();
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
            timestamp: 0, // overridden by add_at
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
        let store = HistoryStore::in_memory();
        store.add_at(1000, entry("Bonjour le monde", "whisper:large-v3", "fr"));

        let results = store.get("", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Bonjour le monde");
        assert_eq!(results[0].model_id, "whisper:large-v3");
        assert_eq!(results[0].language, "fr");
    }

    #[test]
    fn manual_correction_lands_as_final_step_and_becomes_the_text() {
        let store = HistoryStore::in_memory();
        store.add_at(1000, entry("Bonjour le mode", "whisper:large-v3", "fr"));

        store.set_correction(1000, "Bonjour le monde").unwrap();

        let entries = store.get("", 10, None).unwrap();
        assert_eq!(entries[0].text, "Bonjour le monde");
        let steps: Vec<(String, String)> = serde_json::from_str(&entries[0].raw_text).unwrap();
        assert_eq!(steps[0].0, "asr", "l'original reste diffable");
        assert_eq!(steps[0].1, "Bonjour le mode");
        assert_eq!(steps.last().unwrap().0, "manual");
    }

    #[test]
    fn correcting_twice_replaces_the_step() {
        let store = HistoryStore::in_memory();
        store.add_at(1000, entry("Bonjour le mode", "whisper:large-v3", "fr"));

        store.set_correction(1000, "premier essai").unwrap();
        store.set_correction(1000, "Bonjour le monde").unwrap();

        let entries = store.get("", 10, None).unwrap();
        let steps: Vec<(String, String)> = serde_json::from_str(&entries[0].raw_text).unwrap();
        assert_eq!(steps.iter().filter(|(n, _)| n == "manual").count(), 1);
        assert_eq!(entries[0].text, "Bonjour le monde");
    }

    #[test]
    fn rating_defaults_to_zero_and_round_trips() {
        let store = HistoryStore::in_memory();
        store.add_at(1000, entry("Bonjour", "whisper:large-v3", "fr"));
        assert_eq!(store.get("", 10, None).unwrap()[0].rating, 0);

        store.set_rating(1000, -1).unwrap();
        assert_eq!(store.get("", 10, None).unwrap()[0].rating, -1);

        store.set_rating(1000, 0).unwrap();
        assert_eq!(store.get("", 10, None).unwrap()[0].rating, 0);
    }

    #[test]
    fn history_ordered_most_recent_first() {
        let store = HistoryStore::in_memory();
        store.add_at(100, entry("First", "whisper:tiny", "en"));
        store.add_at(300, entry("Third", "whisper:tiny", "en"));
        store.add_at(200, entry("Second", "whisper:tiny", "en"));

        let results = store.get("", 10, None).unwrap();
        assert_eq!(results[0].text, "Third");
        assert_eq!(results[1].text, "Second");
        assert_eq!(results[2].text, "First");
    }

    #[test]
    fn history_count_reflects_total() {
        let store = HistoryStore::in_memory();
        assert_eq!(store.count("").unwrap(), 0);

        store.add_at(100, entry("One", "", ""));
        store.add_at(200, entry("Two", "", ""));
        store.add_at(300, entry("Three", "", ""));
        assert_eq!(store.count("").unwrap(), 3);
    }

    #[test]
    fn history_limit_caps_results() {
        let store = HistoryStore::in_memory();
        for i in 0..20 {
            store.add_at(i, entry(&format!("Entry {}", i), "", ""));
        }

        let results = store.get("", 5, None).unwrap();
        assert_eq!(results.len(), 5);
    }

    // -- Cursor-based pagination (infinite scroll) --

    #[test]
    fn cursor_pagination_returns_older_entries() {
        let store = HistoryStore::in_memory();
        store.add_at(100, entry("Old", "", ""));
        store.add_at(200, entry("Middle", "", ""));
        store.add_at(300, entry("Recent", "", ""));

        // First page: most recent
        let page1 = store.get("", 2, None).unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].text, "Recent");
        assert_eq!(page1[1].text, "Middle");

        // Second page: cursor = timestamp of last entry on page 1
        let cursor = page1[1].timestamp;
        let page2 = store.get("", 2, Some(cursor)).unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].text, "Old");
    }

    #[test]
    fn cursor_past_all_entries_returns_empty() {
        let store = HistoryStore::in_memory();
        store.add_at(100, entry("Only one", "", ""));

        let results = store.get("", 10, Some(50)).unwrap();
        assert!(results.is_empty());
    }

    // -- Search --

    #[test]
    fn search_filters_by_text() {
        let store = HistoryStore::in_memory();
        store.add_at(100, entry("Bonjour le monde", "", "fr"));
        store.add_at(200, entry("Hello world", "", "en"));
        store.add_at(300, entry("Bonsoir tout le monde", "", "fr"));

        let results = store.get("monde", 10, None).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.text.contains("monde")));
    }

    #[test]
    fn search_case_insensitive() {
        let store = HistoryStore::in_memory();
        store.add_at(100, entry("BONJOUR", "", ""));

        // SQLite LIKE is case-insensitive for ASCII
        let results = store.get("bonjour", 10, None).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_count_matches_results() {
        let store = HistoryStore::in_memory();
        store.add_at(100, entry("Bonjour", "", ""));
        store.add_at(200, entry("Hello", "", ""));
        store.add_at(300, entry("Bon appétit", "", ""));

        assert_eq!(store.count("Bon").unwrap(), 2);
        assert_eq!(store.count("Hello").unwrap(), 1);
        assert_eq!(store.count("xyz").unwrap(), 0);
    }

    #[test]
    fn search_with_cursor() {
        let store = HistoryStore::in_memory();
        store.add_at(100, entry("Bonjour A", "", ""));
        store.add_at(200, entry("Hello", "", ""));
        store.add_at(300, entry("Bonjour B", "", ""));

        let page1 = store.get("Bonjour", 1, None).unwrap();
        assert_eq!(page1.len(), 1);
        assert_eq!(page1[0].text, "Bonjour B");

        let page2 = store.get("Bonjour", 1, Some(page1[0].timestamp)).unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].text, "Bonjour A");
    }

    // -- Deletion --

    #[test]
    fn delete_single_entry() {
        let store = HistoryStore::in_memory();
        store.add_at(100, entry("Keep me", "", ""));
        store.add_at(200, entry("Delete me", "", ""));

        store.delete_entry(200).unwrap();
        let results = store.get("", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "Keep me");
    }

    #[test]
    fn delete_day_removes_24h_window() {
        let store = HistoryStore::in_memory();
        let day_start: u64 = 1700000000; // some day
        store.add_at(day_start + 100, entry("Morning", "", ""));
        store.add_at(day_start + 50000, entry("Afternoon", "", ""));
        store.add_at(day_start + 86400 + 100, entry("Next day", "", ""));
        store.add_at(day_start - 100, entry("Previous day", "", ""));

        store.delete_day(day_start).unwrap();

        let results = store.get("", 10, None).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|e| e.text == "Next day"));
        assert!(results.iter().any(|e| e.text == "Previous day"));
    }

    #[test]
    fn clear_history_removes_all() {
        let store = HistoryStore::in_memory();
        for i in 0..10 {
            store.add_at(i * 100, entry(&format!("Entry {}", i), "", ""));
        }
        assert_eq!(store.count("").unwrap(), 10);

        store.clear().unwrap();
        assert_eq!(store.count("").unwrap(), 0);
        assert!(store.get("", 10, None).unwrap().is_empty());
    }

    // -- Metadata preservation --

    #[test]
    fn history_preserves_pipeline_metadata() {
        let store = HistoryStore::in_memory();
        store.add_at(100, HistoryEntry {
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

        let results = store.get("", 10, None).unwrap();
        let e = &results[0];
        assert_eq!(e.cleanup_model_id, "correction:gec-t5-small");
        assert!(e.hallucination_filter);
        assert!(e.vad_trimmed);
        assert_eq!(e.punctuation_model_id, "punctuation:pcs");
        assert!(e.spellcheck);
        assert!(e.disfluency_removal);
        assert!(e.itn);
    }
}
