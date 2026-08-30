// Re-export only the types that other modules access via `crate::state::`.
pub use jona_types::{GpuMode, RecordingMode};

use crate::history::HistoryStore;
use jona_types::{
    AudioFlags, ContextMap, DownloadState, Preferences, Provider,
    RuntimeState, load_api_keys_from_keyring, prefs_path,
};
use std::sync::{Arc, Mutex};

// -- Main AppState --

pub struct AppState {
    pub runtime: Mutex<RuntimeState>,
    pub download: Arc<Mutex<DownloadState>>,
    pub settings: Mutex<Preferences>,
    pub history: HistoryStore,
    pub tray_menu: Mutex<Option<crate::ui::tray::TrayMenuState>>,
    /// Dynamic context map for all engine inference contexts (ASR + cleanup).
    /// Replaces the old typed `InferenceContexts` with type-erased storage.
    pub contexts: ContextMap,
    /// Lock-free flags for spectrum emitter hot path.
    pub audio_flags: AudioFlags,
    /// Providers auto-detected from other tools (ephemeral, not saved to prefs).
    pub detected_providers: Mutex<Vec<Provider>>,
}

/// Load preferences with migration support.
fn load_preferences() -> Preferences {
    // Rename WhisperDictate/ → JonaWhisper/ before reading config
    crate::migrations::migrate_data_directory();

    let path = prefs_path();
    let data = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return serde_json::from_str("{}").unwrap_or_default(),
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
            history: HistoryStore::open(),
            tray_menu: Mutex::new(None),
            contexts: ContextMap::new(),
            audio_flags: AudioFlags::default(),
            detected_providers: Mutex::new(vec![]),
        }
    }
}

impl AppState {
    /// Run all registered credential detectors and populate `detected_providers`.
    /// Detectors whose providers are ALL explicitly disabled are skipped to avoid
    /// unnecessary Keychain popups on macOS.
    pub fn run_detection(&self) {
        // Build set of detector IDs to skip: a detector is skipped when every
        // provider it previously produced has been explicitly disabled by the user.
        let skip_owned: std::collections::HashSet<String> = {
            // Use persisted maps (survive restarts) to decide which detectors to skip.
            // A detector is skipped when ALL its providers are explicitly disabled.
            let settings = self.settings.lock().unwrap();
            let enabled_map = &settings.detected_enabled;
            let sources_map = &settings.detected_sources;
            let mut detector_ids: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
            for (provider_id, &enabled) in enabled_map {
                if let Some(detector_id) = sources_map.get(provider_id) {
                    let all_disabled = detector_ids.entry(detector_id.clone()).or_insert(true);
                    if enabled {
                        *all_disabled = false;
                    }
                }
            }
            detector_ids.into_iter()
                .filter(|(_, all_disabled)| *all_disabled)
                .map(|(id, _)| id)
                .collect()
        };
        let skip: std::collections::HashSet<&str> = skip_owned.iter().map(|s| s.as_str()).collect();
        let results = jona_provider::detect_all(&skip);
        // Restore persisted enabled states for detected providers
        let enabled_states: std::collections::HashMap<String, bool> = self.settings.lock().unwrap()
            .detected_enabled.clone();
        let mut detected = Vec::new();
        let mut id_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (cred, detector_id) in results {
            let base_id = format!("auto-{}-{}", detector_id, cred.kind);
            let count = id_counts.entry(base_id.clone()).or_insert(0);
            let id = if *count == 0 { base_id.clone() } else { format!("{}-{}", base_id, count) };
            *count += 1;
            let preset = jona_provider::preset(cred.kind);
            let preset_name = preset.map(|p| p.display_name).unwrap_or(cred.kind);
            let url = if cred.url.is_empty() {
                preset.map(|p| p.base_url.to_string()).unwrap_or_default()
            } else {
                cred.url
            };
            let enabled = enabled_states.get(&id).copied().unwrap_or(false);
            detected.push(Provider {
                id,
                name: format!("{} ({})", preset_name, cred.source_label),
                kind: cred.kind.to_string(),
                url,
                api_key: cred.api_key,
                allow_insecure: false,
                cached_models: vec![],
                supports_asr: preset.map(|p| p.supports_asr).unwrap_or(false),
                supports_llm: preset.map(|p| p.supports_llm).unwrap_or(false),
                api_format: None,
                extra: cred.extra,
                enabled,
                source: Some(detector_id.to_string()),
            });
        }
        log::info!("Auto-detection: {} provider(s) found", detected.len());

        // Persist detector sources and prune orphan entries
        let detected_ids: std::collections::HashSet<&str> = detected.iter().map(|p| p.id.as_str()).collect();
        let mut s = self.settings.lock().unwrap();
        // Update detected_sources mapping (provider_id → detector_id)
        for p in &detected {
            if let Some(source) = &p.source {
                s.detected_sources.insert(p.id.clone(), source.clone());
            }
        }
        // Prune orphan entries (detectors that no longer return credentials)
        let before = s.detected_enabled.len() + s.detected_sources.len();
        s.detected_enabled.retain(|id, _| detected_ids.contains(id.as_str()));
        s.detected_sources.retain(|id, _| detected_ids.contains(id.as_str()));
        let after = s.detected_enabled.len() + s.detected_sources.len();
        drop(s);
        if before != after || !detected.is_empty() {
            self.save_preferences();
        }

        *self.detected_providers.lock().unwrap() = detected;
    }

    /// Find a provider by ID across both manual and detected providers.
    /// For detected providers, re-reads credentials from the source (e.g. Keychain)
    /// to get fresh tokens that may have been rotated.
    pub fn find_provider(&self, id: &str) -> Option<Provider> {
        let s = self.settings.lock().unwrap();
        if let Some(p) = s.providers.iter().find(|p| p.id == id) {
            return Some(p.clone());
        }
        drop(s);
        let mut provider = self.detected_providers.lock().unwrap()
            .iter().find(|p| p.id == id).cloned();
        // Drop the mutex before doing Keychain I/O
        if let Some(ref mut p) = provider {
            // Re-read fresh credentials from the detector (handles token rotation)
            if let Some(source) = &p.source {
                if let Some(cred) = jona_provider::refresh_credential(source, &p.kind) {
                    p.api_key = cred.api_key;
                }
            }
        }
        provider
    }

    /// Save current preferences to disk.
    pub fn save_preferences(&self) {
        let settings = self.settings.lock().unwrap();
        log::debug!("save_preferences: {} provider(s)", settings.providers.len());
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
}

#[cfg(test)]
impl AppState {
    /// Create an AppState with in-memory SQLite for testing.
    pub(crate) fn test_instance() -> Self {
        Self {
            runtime: Mutex::new(RuntimeState::default()),
            download: Arc::new(Mutex::new(DownloadState::default())),
            settings: Mutex::new(Preferences::default()),
            history: HistoryStore::in_memory(),
            tray_menu: Mutex::new(None),
            contexts: ContextMap::new(),
            audio_flags: AudioFlags::default(),
            detected_providers: Mutex::new(vec![]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
