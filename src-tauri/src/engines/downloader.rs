use super::{ASRModel, DownloadType};
use crate::state::AppState;
use futures_util::StreamExt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};

static DOWNLOAD_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

const PENDING_DIR: &str = ".local/share/whisper-dictate";

fn pending_download_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(PENDING_DIR)
        .join(".pending-download")
}

pub async fn download_model(
    app: AppHandle,
    state: Arc<AppState>,
    model: ASRModel,
) -> bool {
    // Create pending dir and write pending state
    let pending_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(PENDING_DIR);
    let _ = fs::create_dir_all(&pending_dir);
    let _ = fs::write(pending_download_path(), &model.id);

    {
        let mut dl = state.download.lock().unwrap();
        dl.model_id = Some(model.id.clone());
        dl.progress = 0.0;
    }

    let success = match &model.download_type {
        DownloadType::RemoteAPI | DownloadType::System => true,
        DownloadType::SingleFile => {
            download_single_file(&app, &state, &model).await
        }
    };

    clear_pending_state(&model);
    {
        let mut dl = state.download.lock().unwrap();
        dl.model_id = None;
        dl.progress = 0.0;
    }

    success
}

async fn download_single_file(
    app: &AppHandle,
    state: &AppState,
    model: &ASRModel,
) -> bool {
    let storage_dir = shellexpand::tilde(&model.storage_dir).to_string();
    let _ = fs::create_dir_all(&storage_dir);

    let client = &*DOWNLOAD_CLIENT;

    let dest_path = model.local_path();
    let response = match client.get(&model.url).send().await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Download failed: {}", e);
            return false;
        }
    };

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let tmp_path = std::env::temp_dir().join(format!("whisper_dl_{}", uuid_simple()));

    let mut file = match fs::File::create(&tmp_path) {
        Ok(f) => f,
        Err(e) => {
            log::error!("Failed to create temp file: {}", e);
            return false;
        }
    };

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                if file.write_all(&bytes).is_err() {
                    return false;
                }
                downloaded += bytes.len() as u64;
                if total_size > 0 {
                    let progress = downloaded as f64 / total_size as f64;
                    state.download.lock().unwrap().progress = progress;
                    let _ = app.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
                        "model_id": model.id,
                        "progress": progress,
                    }));
                }
            }
            Err(e) => {
                log::error!("Download stream error: {}", e);
                let _ = fs::remove_file(&tmp_path);
                return false;
            }
        }
    }

    // Move to final destination
    if dest_path.exists() {
        let _ = fs::remove_file(&dest_path);
    }
    match fs::rename(&tmp_path, &dest_path) {
        Ok(()) => true,
        Err(_) => {
            // rename might fail across filesystems, try copy
            match fs::copy(&tmp_path, &dest_path) {
                Ok(_) => {
                    let _ = fs::remove_file(&tmp_path);
                    true
                }
                Err(e) => {
                    log::error!("Failed to move downloaded file: {}", e);
                    let _ = fs::remove_file(&tmp_path);
                    false
                }
            }
        }
    }
}

pub fn delete_model(model: &ASRModel) -> bool {
    if !model.is_downloaded() {
        return false;
    }
    let path = model.local_path();
    if path.is_dir() {
        fs::remove_dir_all(&path).is_ok()
    } else {
        fs::remove_file(&path).is_ok()
    }
}

fn clear_pending_state(_model: &ASRModel) {
    let _ = fs::remove_file(pending_download_path());
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{:x}{:x}", t.as_secs(), t.subsec_nanos())
}
