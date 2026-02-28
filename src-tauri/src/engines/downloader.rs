use super::{ASRModel, DownloadType};
use crate::state::AppState;
use futures_util::StreamExt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};

static DOWNLOAD_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

fn pending_download_path() -> PathBuf {
    crate::state::config_dir().join(".pending-download")
}

/// Stable partial file path for a model (deterministic, survives app restart).
fn partial_path(model: &ASRModel) -> PathBuf {
    let hash = model.id.replace([':', '/'], "_");
    let storage_dir = shellexpand::tilde(&model.storage_dir).to_string();
    PathBuf::from(storage_dir).join(format!(".{}.partial", hash))
}

/// Returns download progress (0.0–0.99) if a `.partial` file exists for this model.
pub fn partial_progress(model: &ASRModel) -> Option<f64> {
    if model.size == 0 { return None; }
    let size = fs::metadata(partial_path(model)).map(|m| m.len()).unwrap_or(0);
    if size > 0 {
        Some((size as f64 / model.size as f64).min(0.99))
    } else {
        None
    }
}

/// Delete the `.partial` file for a model (used when cancelling a paused download).
pub fn delete_partial(model: &ASRModel) {
    let path = partial_path(model);
    if path.exists() {
        let _ = fs::remove_file(&path);
        log::info!("Deleted partial file for {}", model.id);
    }
}

pub async fn download_model(
    app: AppHandle,
    state: Arc<AppState>,
    model: ASRModel,
) -> bool {
    // Write pending state
    let pending = pending_download_path();
    if let Some(parent) = pending.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&pending, &model.id);

    // Compute initial progress from partial file (avoids 0% → X% flash on resume)
    let initial_progress = if model.size > 0 {
        let existing = fs::metadata(partial_path(&model)).map(|m| m.len()).unwrap_or(0);
        if existing > 0 {
            (existing as f64 / model.size as f64).min(0.99)
        } else {
            0.0
        }
    } else {
        0.0
    };

    {
        let mut dl = state.download.lock().unwrap();
        dl.model_id = Some(model.id.clone());
        dl.progress = initial_progress;
    }

    let _ = app.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
        "model_id": model.id,
        "progress": initial_progress,
    }));

    // Clone cancel flags before download (checked in the loop)
    let cancel_flag = state.download.lock().unwrap().cancel_requested.clone();
    let delete_flag = state.download.lock().unwrap().delete_partial.clone();

    let success = match &model.download_type {
        DownloadType::RemoteAPI | DownloadType::System => true,
        DownloadType::SingleFile => {
            download_single_file(&app, &state, &model, &cancel_flag).await
        }
    };

    // If cancel was requested and delete_partial is set, remove the .partial file
    let was_cancelled = cancel_flag.load(Ordering::SeqCst);
    if was_cancelled && delete_flag.load(Ordering::SeqCst) {
        let tmp_path = partial_path(&model);
        let _ = fs::remove_file(&tmp_path);
        log::info!("Cancelled download for {} — partial file deleted", model.id);
    } else if was_cancelled {
        log::info!("Stopped download for {} — partial file kept for resume", model.id);
    }

    // Reset flags
    cancel_flag.store(false, Ordering::SeqCst);
    delete_flag.store(false, Ordering::SeqCst);

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
    cancel_flag: &std::sync::atomic::AtomicBool,
) -> bool {
    let storage_dir = shellexpand::tilde(&model.storage_dir).to_string();
    let _ = fs::create_dir_all(&storage_dir);

    let client = &*DOWNLOAD_CLIENT;
    let dest_path = model.local_path();
    let tmp_path = partial_path(model);

    // Check for existing partial download
    let existing_size = fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);

    // Build request with Range header if resuming
    let mut request = client.get(&model.url);
    if existing_size > 0 {
        log::info!("Resuming download for {} from {} bytes", model.id, existing_size);
        request = request.header("Range", format!("bytes={}-", existing_size));
    }

    let response = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Download failed: {}", e);
            return false;
        }
    };

    let status = response.status();

    // 416 = Range not satisfiable (partial file corrupted or larger than remote)
    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        log::warn!("Server returned 416 for {}, deleting partial and retrying", model.id);
        let _ = fs::remove_file(&tmp_path);
        return Box::pin(download_single_file(app, state, model, cancel_flag)).await;
    }

    // 206 = partial content (resume accepted), 200 = full file (server ignores Range)
    let (resumed, total_size) = if status == reqwest::StatusCode::PARTIAL_CONTENT {
        let remaining = response.content_length().unwrap_or(0);
        log::info!("Resuming {} — server accepted Range ({} + {} bytes)", model.id, existing_size, remaining);
        (true, existing_size + remaining)
    } else {
        if existing_size > 0 {
            log::info!("Server does not support Range for {} — restarting download", model.id);
        }
        (false, response.content_length().unwrap_or(0))
    };

    let mut downloaded: u64 = if resumed { existing_size } else { 0 };

    // Open file: append if resuming, create if fresh
    let mut file = if resumed {
        match fs::OpenOptions::new().append(true).open(&tmp_path) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to open partial file for append: {}", e);
                return false;
            }
        }
    } else {
        match fs::File::create(&tmp_path) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to create temp file: {}", e);
                return false;
            }
        }
    };

    // Emit initial progress if resuming
    if resumed && total_size > 0 {
        let progress = downloaded as f64 / total_size as f64;
        state.download.lock().unwrap().progress = progress;
        let _ = app.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
            "model_id": model.id,
            "progress": progress,
        }));
    }

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancel_flag.load(Ordering::SeqCst) {
            log::info!("Download cancelled for {}", model.id);
            return false;
        }
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
                // Keep partial file for resume on next attempt
                return false;
            }
        }
    }

    // Verify size if known from model catalog
    if model.size > 0 && downloaded < model.size / 2 {
        log::error!("Downloaded size ({}) is suspiciously small for model {} (expected ~{})",
            downloaded, model.id, model.size);
        let _ = fs::remove_file(&tmp_path);
        return false;
    }

    // Move to final destination, remove partial file
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
