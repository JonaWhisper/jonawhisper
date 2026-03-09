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
        "disfluency_removal_enabled": s.disfluency_removal_enabled,
        "itn_enabled": s.itn_enabled,
        "spellcheck_enabled": s.spellcheck_enabled,
        "theme": s.theme,
        "log_level": s.log_level,
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
            "disfluency_removal_enabled" => s.disfluency_removal_enabled = value == "true",
            "itn_enabled" => s.itn_enabled = value == "true",
            "spellcheck_enabled" => s.spellcheck_enabled = value == "true",
            "theme" => s.theme = value.clone(),
            "log_level" => {
                s.log_level = value.clone();
                let level = parse_log_level(&value);
                log::set_max_level(level);
                log::info!("Log level set to {}", level);
            }
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

#[tauri::command]
pub fn get_launch_at_login_status() -> String {
    platform::get_launch_at_login_status().to_string()
}

#[tauri::command]
pub fn set_launch_at_login(enabled: bool) -> Result<String, String> {
    platform::set_launch_at_login(enabled).map(|s| s.to_string())
}
