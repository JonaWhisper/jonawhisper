use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::path::PathBuf;

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

impl Default for AppState {
    fn default() -> Self {
        Self {
            is_recording: Mutex::new(false),
            is_transcribing: Mutex::new(false),
            transcription_queue: Mutex::new(Vec::new()),
            downloading_model_id: Mutex::new(None),
            download_progress: Mutex::new(0.0),
            transcription_cancelled: Mutex::new(false),
            selected_model_id: Mutex::new("whisper:large-v3-turbo".to_string()),
            selected_language: Mutex::new("auto".to_string()),
            post_processing_enabled: Mutex::new(true),
            hotkey_option: Mutex::new("right_command".to_string()),
            selected_input_device_uid: Mutex::new(None),
            transcription_history: Mutex::new(Vec::new()),
            api_servers: Mutex::new(Vec::new()),
        }
    }
}

impl AppState {
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
