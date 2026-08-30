use crate::platform;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub fn parse_log_level(s: &str) -> log::LevelFilter {
    match s {
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,
    }
}

#[tauri::command]
pub fn get_system_locale(state: tauri::State<'_, Arc<AppState>>) -> String {
    let locale = state.settings.lock().unwrap().app_locale.clone();
    crate::resolve_locale(&locale)
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> serde_json::Value {
    let s = state.settings.lock().unwrap();
    serde_json::json!({
        "app_locale": s.app_locale,
        "hallucination_filter_enabled": s.hallucination_filter_enabled,
        "hotkey": s.hotkey_option,
        "selected_input_device_uid": s.selected_input_device_uid,
        "selected_model_id": s.selected_model_id,
        "selected_language": s.selected_language,
        "cancel_shortcut": s.cancel_shortcut,
        "recording_mode": s.recording_mode,
        "text_cleanup_enabled": s.text_cleanup_enabled,
        "punctuation_model_id": s.punctuation_model_id,
        "cleanup_model_id": s.cleanup_model_id,
        "llm_provider_id": s.llm_provider_id,
        "llm_model": s.llm_model,
        "asr_cloud_model": s.asr_cloud_model,
        "gpu_mode": s.gpu_mode,
        "llm_max_tokens": s.llm_max_tokens,
        "audio_ducking_enabled": s.audio_ducking_enabled,
        "audio_ducking_level": s.audio_ducking_level,
        "vad_enabled": s.vad_enabled,
        "live_preview_enabled": s.live_preview_enabled,
        "live_preview_model_id": s.live_preview_model_id,
        "live_preview_max_lines": s.live_preview_max_lines,
        "disfluency_removal_enabled": s.disfluency_removal_enabled,
        "itn_enabled": s.itn_enabled,
        "spellcheck_enabled": s.spellcheck_enabled,
        "auto_release_memory": s.auto_release_memory,
        "theme": s.theme,
        "log_level": s.log_level,
        "log_retention": s.log_retention,
    })
}

#[tauri::command]
pub fn set_setting(
    key: String,
    value: String,
    state: tauri::State<'_, Arc<AppState>>,
    hotkey_sender: tauri::State<'_, crate::HotkeyUpdateSender>,
    app: AppHandle,
) {
    use crate::platform::hotkey;

    log::info!("set_setting: key={}", key);
    {
        let mut s = state.settings.lock().unwrap();
        match key.as_str() {
            "app_locale" => {
                s.app_locale = value.clone();
                let lang = crate::resolve_locale(&value);
                rust_i18n::set_locale(&lang);
            }
            "hallucination_filter_enabled" => s.hallucination_filter_enabled = value == "true",
            "hotkey" => s.hotkey_option = value.clone(),
            "cancel_shortcut" => s.cancel_shortcut = value.clone(),
            "recording_mode" => s.recording_mode = crate::state::RecordingMode::parse(&value),
            "selected_input_device_uid" => {
                s.selected_input_device_uid = if value.is_empty() { None } else { Some(value.clone()) };
            }
            "selected_model_id" => s.selected_model_id = value.clone(),
            "selected_language" => s.selected_language = value.clone(),
            "text_cleanup_enabled" => s.text_cleanup_enabled = value == "true",
            "punctuation_model_id" => s.punctuation_model_id = value.clone(),
            "cleanup_model_id" => s.cleanup_model_id = value.clone(),
            "llm_provider_id" => s.llm_provider_id = value.clone(),
            "llm_model" => s.llm_model = value.clone(),
            "asr_cloud_model" => s.asr_cloud_model = value.clone(),
            "gpu_mode" => s.gpu_mode = crate::state::GpuMode::parse(&value),
            "llm_max_tokens" => s.llm_max_tokens = value.parse::<u32>().unwrap_or(256),
            "audio_ducking_enabled" => s.audio_ducking_enabled = value == "true",
            "audio_ducking_level" => s.audio_ducking_level = value.parse().unwrap_or(0.8),
            "vad_enabled" => s.vad_enabled = value == "true",
            "live_preview_enabled" => s.live_preview_enabled = value == "true",
            "live_preview_model_id" => s.live_preview_model_id = value.to_string(),
            "live_preview_max_lines" => s.live_preview_max_lines = value.parse().unwrap_or(5).clamp(1, 10),
            "disfluency_removal_enabled" => s.disfluency_removal_enabled = value == "true",
            "itn_enabled" => s.itn_enabled = value == "true",
            "spellcheck_enabled" => s.spellcheck_enabled = value == "true",
            "auto_release_memory" => s.auto_release_memory = value == "true",
            "theme" => s.theme = value.clone(),
            "log_level" => {
                s.log_level = value.clone();
                let level = parse_log_level(&value);
                log::set_max_level(level);
                log::info!("Log level set to {}", level);
            }
            "log_retention" => s.log_retention = value.clone(),
            _ => {
                log::warn!("Unknown setting key: {}", key);
                return;
            }
        }
    }
    // Invalidate cached contexts when model or GPU mode changes
    if key == "selected_model_id" || key == "gpu_mode" || key == "cleanup_model_id" || key == "punctuation_model_id" {
        state.contexts.invalidate_all();
    }
    // Send hotkey updates outside the settings lock
    match key.as_str() {
        "hotkey" => {
            let shortcut = hotkey::Shortcut::parse(&value);
            let _ = hotkey_sender.0.send(hotkey::HotkeyUpdate::SetRecordShortcut(shortcut));
        }
        "cancel_shortcut" => {
            let shortcut = hotkey::Shortcut::parse(&value);
            let _ = hotkey_sender.0.send(hotkey::HotkeyUpdate::SetCancelShortcut(shortcut));
        }
        _ => {}
    }
    state.save_preferences();
    if key == "app_locale" {
        crate::ui::tray::update_tray_labels(&app);
    }
    let _ = app.emit(crate::events::SETTINGS_CHANGED, &key);
}

/// A user dictionary entry (word or ITN mapping).
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct UserDictEntry {
    /// For words: the word. For mappings: "pattern=replacement".
    pub value: String,
    /// "word" or "mapping"
    pub kind: String,
}

#[tauri::command]
pub fn get_user_dict() -> Vec<UserDictEntry> {
    let path = crate::cleanup::symspell_correct::user_dict_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            if line.contains('=') {
                Some(UserDictEntry { value: line.to_string(), kind: "mapping".to_string() })
            } else {
                // word or word\tfreq — extract word part
                let word = line.split('\t').next().unwrap_or(line).trim();
                Some(UserDictEntry { value: word.to_string(), kind: "word".to_string() })
            }
        })
        .collect()
}

#[tauri::command]
pub fn save_user_dict(entries: Vec<UserDictEntry>) -> Result<(), String> {
    let path = crate::cleanup::symspell_correct::user_dict_path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;

    let mut content = String::new();
    for entry in &entries {
        if !entry.value.trim().is_empty() {
            content.push_str(entry.value.trim());
            content.push('\n');
        }
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    log::info!("User dict: saved {} entries", entries.len());
    Ok(())
}

/// A word the user dictates repeatedly that no dictionary knows.
#[derive(serde::Serialize)]
pub struct DictSuggestion {
    pub word: String,
    pub count: u32,
}

/// Words worth protecting are the ones the user actually says and no dictionary
/// covers — spell-check rewrites exactly those. Suggestions only: a recurring ASR
/// mistake looks identical here, and freezing one into the dictionary is worse
/// than the rewrite it would prevent.
#[tauri::command]
pub fn suggest_user_dict_words(
    language: String,
    min_count: u32,
    state: tauri::State<'_, std::sync::Arc<crate::state::AppState>>,
) -> Result<Vec<DictSuggestion>, String> {
    use crate::cleanup::symspell_correct::{is_known_word, word_boundaries, MIN_CORRECTION_LEN};
    use std::collections::HashMap;

    let (texts, language) = {
        let db = state.history_db.lock().unwrap_or_else(|e| e.into_inner());
        // "auto" never reaches the history rows, which store the resolved language.
        let language = if language.is_empty() || language == "auto" {
            db.query_row(
                "SELECT language FROM history WHERE language <> '' \
                 GROUP BY language ORDER BY COUNT(*) DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(|_| "no dictation history yet".to_string())?
        } else {
            language
        };
        let mut stmt = db
            .prepare("SELECT text FROM history WHERE language = ?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&language], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        (rows.filter_map(Result::ok).collect::<Vec<String>>(), language)
    };

    let mut counts: HashMap<String, u32> = HashMap::new();
    for text in &texts {
        for (_, word) in word_boundaries(text) {
            if word.chars().count() >= MIN_CORRECTION_LEN && word.chars().any(|c| c.is_alphabetic())
            {
                *counts.entry(word.to_lowercase()).or_default() += 1;
            }
        }
    }

    let mut out: Vec<DictSuggestion> = counts
        .into_iter()
        .filter(|(word, count)| *count >= min_count && !is_known_word(word, &language))
        .map(|(word, count)| DictSuggestion { word, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.word.cmp(&b.word)));
    log::info!("User dict: {} suggestions from {} dictations", out.len(), texts.len());
    Ok(out)
}

#[tauri::command]
pub fn open_logs_folder() {
    let log_dir = jona_types::config_dir().join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    if let Ok(mut child) = std::process::Command::new("open").arg(&log_dir).spawn() {
        std::thread::spawn(move || { let _ = child.wait(); });
    }
}

#[tauri::command]
pub fn get_launch_at_login_status() -> String {
    platform::get_launch_at_login_status().to_string()
}

#[tauri::command]
pub fn set_launch_at_login(enabled: bool) -> Result<String, String> {
    platform::set_launch_at_login(enabled).map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_level_trace() {
        assert_eq!(parse_log_level("trace"), log::LevelFilter::Trace);
    }

    #[test]
    fn parse_log_level_debug() {
        assert_eq!(parse_log_level("debug"), log::LevelFilter::Debug);
    }

    #[test]
    fn parse_log_level_info() {
        assert_eq!(parse_log_level("info"), log::LevelFilter::Info);
    }

    #[test]
    fn parse_log_level_warn() {
        assert_eq!(parse_log_level("warn"), log::LevelFilter::Warn);
    }

    #[test]
    fn parse_log_level_error() {
        assert_eq!(parse_log_level("error"), log::LevelFilter::Error);
    }

    #[test]
    fn parse_log_level_unknown_defaults_to_info() {
        assert_eq!(parse_log_level("verbose"), log::LevelFilter::Info);
        assert_eq!(parse_log_level(""), log::LevelFilter::Info);
        assert_eq!(parse_log_level("INFO"), log::LevelFilter::Info);
    }
}
