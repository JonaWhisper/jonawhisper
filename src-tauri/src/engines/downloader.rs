use super::{ASRModel, DownloadFile, DownloadType};
use crate::state::{ActiveDownload, AppState};
use futures_util::StreamExt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Returns download progress (0.0–0.99) if a partial download exists for this model.
pub fn partial_progress(model: &ASRModel) -> Option<f64> {
    if model.size == 0 { return None; }

    match &model.download_type {
        DownloadType::MultiFile { files } => {
            // Sum completed files + any current partial file
            let model_dir = model.local_path();
            let mut completed_bytes: u64 = 0;
            for f in files {
                let file_path = model_dir.join(&f.filename);
                if file_path.exists() {
                    completed_bytes += f.size;
                } else {
                    // Check for partial
                    let p = multi_file_partial_path(model, &f.filename);
                    completed_bytes += fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                }
            }
            let total: u64 = files.iter().map(|f| f.size).sum();
            if completed_bytes > 0 && total > 0 {
                Some((completed_bytes as f64 / total as f64).min(0.99))
            } else {
                None
            }
        }
        _ => {
            let size = fs::metadata(partial_path(model)).map(|m| m.len()).unwrap_or(0);
            if size > 0 {
                Some((size as f64 / model.size as f64).min(0.99))
            } else {
                None
            }
        }
    }
}

/// Partial file path for a specific file within a multi-file model directory.
fn multi_file_partial_path(model: &ASRModel, filename: &str) -> PathBuf {
    let hash = filename.replace(['/', '.'], "_");
    model.local_path().join(format!(".{}.partial", hash))
}

/// Delete the `.partial` file(s) for a model (used when cancelling a paused download).
pub fn delete_partial(model: &ASRModel) {
    match &model.download_type {
        DownloadType::MultiFile { files } => {
            for f in files {
                let p = multi_file_partial_path(model, &f.filename);
                if p.exists() {
                    let _ = fs::remove_file(&p);
                }
            }
            log::info!("Deleted partial files for {}", model.id);
        }
        _ => {
            let path = partial_path(model);
            if path.exists() {
                let _ = fs::remove_file(&path);
                log::info!("Deleted partial file for {}", model.id);
            }
        }
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

    // Compute initial progress from partial file(s) (avoids 0% → X% flash on resume)
    let initial_progress = partial_progress(&model).unwrap_or(0.0);

    // Register this download in the per-model HashMap
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let delete_flag = Arc::new(AtomicBool::new(false));
    {
        let mut dl = state.download.lock().unwrap();
        dl.active.insert(model.id.clone(), ActiveDownload {
            progress: initial_progress,
            cancel_requested: cancel_flag.clone(),
            delete_partial: delete_flag.clone(),
        });
    }

    let _ = app.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
        "model_id": model.id,
        "progress": initial_progress,
    }));

    let success = match &model.download_type {
        DownloadType::RemoteAPI | DownloadType::System => true,
        DownloadType::SingleFile => {
            download_single_file(&app, &state, &model, &cancel_flag).await
        }
        DownloadType::MultiFile { files } => {
            download_multi_file(&app, &state, &model, files, &cancel_flag).await
        }
    };

    // If cancel was requested and delete_partial is set, clean up partial files
    let was_cancelled = cancel_flag.load(Ordering::SeqCst);
    if was_cancelled && delete_flag.load(Ordering::SeqCst) {
        delete_partial(&model);
        // For multi-file, also remove the directory if partially created
        if matches!(&model.download_type, DownloadType::MultiFile { .. }) {
            let dir = model.local_path();
            if dir.is_dir() {
                let _ = fs::remove_dir_all(&dir);
            }
        }
        log::info!("Cancelled download for {} — partial files deleted", model.id);
    } else if was_cancelled {
        log::info!("Stopped download for {} — partial file kept for resume", model.id);
    }

    clear_pending_state(&model);
    // Remove this download from the HashMap
    {
        let mut dl = state.download.lock().unwrap();
        dl.active.remove(&model.id);
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

    // Emit helper — sends progress + size + speed
    let emit_progress = |app: &AppHandle, state: &AppState, model_id: &str,
                         downloaded: u64, total: u64, speed: u64| {
        let progress = if total > 0 { downloaded as f64 / total as f64 } else { 0.0 };
        if let Some(entry) = state.download.lock().unwrap().active.get_mut(model_id) {
            entry.progress = progress;
        }
        let _ = app.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
            "model_id": model_id,
            "progress": progress,
            "downloaded": downloaded,
            "total_size": total,
            "speed": speed,
        }));
    };

    // Emit initial progress if resuming
    if resumed && total_size > 0 {
        emit_progress(app, state, &model.id, downloaded, total_size, 0);
    }

    // Throttle: emit at most every 250ms
    let mut last_emit_time = std::time::Instant::now();
    let mut last_emit_bytes = downloaded;

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
                    let now = std::time::Instant::now();
                    let elapsed = now.duration_since(last_emit_time);
                    let is_done = downloaded >= total_size;
                    if elapsed >= std::time::Duration::from_millis(250) || is_done {
                        let speed = if elapsed.as_secs_f64() > 0.0 {
                            ((downloaded - last_emit_bytes) as f64 / elapsed.as_secs_f64()) as u64
                        } else { 0 };
                        emit_progress(app, state, &model.id, downloaded, total_size, speed);
                        last_emit_time = now;
                        last_emit_bytes = downloaded;
                    }
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

/// Download multiple files into a model directory, with streaming progress and resume support.
/// Writes a `.complete` marker (from `model.download_marker`) when all files are done.
async fn download_multi_file(
    app: &AppHandle,
    state: &AppState,
    model: &ASRModel,
    files: &[DownloadFile],
    cancel_flag: &AtomicBool,
) -> bool {
    let model_dir = model.local_path();
    let _ = fs::create_dir_all(&model_dir);

    let total_size: u64 = files.iter().map(|f| f.size).sum();

    // Emit helper for overall multi-file progress
    let emit_multi_progress = |app: &AppHandle, state: &AppState, model_id: &str,
                                overall_downloaded: u64, total: u64, speed: u64| {
        let progress = if total > 0 { overall_downloaded as f64 / total as f64 } else { 0.0 };
        if let Some(entry) = state.download.lock().unwrap().active.get_mut(model_id) {
            entry.progress = progress;
        }
        let _ = app.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
            "model_id": model_id,
            "progress": progress,
            "downloaded": overall_downloaded,
            "total_size": total,
            "speed": speed,
        }));
    };

    // Bytes already completed from previous files
    let mut cumulative_completed: u64 = 0;

    for df in files {
        let dest_path = model_dir.join(&df.filename);

        // Skip already-completed files
        if dest_path.exists() {
            cumulative_completed += df.size;
            emit_multi_progress(app, state, &model.id, cumulative_completed, total_size, 0);
            continue;
        }

        let tmp_path = multi_file_partial_path(model, &df.filename);
        let existing_size = fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);

        let client = &*DOWNLOAD_CLIENT;
        let mut request = client.get(&df.url);
        if existing_size > 0 {
            log::info!("Resuming multi-file {} from {} bytes", df.filename, existing_size);
            request = request.header("Range", format!("bytes={}-", existing_size));
        }

        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                log::error!("Download failed for {}: {}", df.filename, e);
                return false;
            }
        };

        let status = response.status();

        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            log::warn!("Server returned 416 for {}, restarting", df.filename);
            let _ = fs::remove_file(&tmp_path);
            // Retry this file by recursing (rare case)
            return Box::pin(download_multi_file(app, state, model, files, cancel_flag)).await;
        }

        let (resumed, file_total) = if status == reqwest::StatusCode::PARTIAL_CONTENT {
            let remaining = response.content_length().unwrap_or(0);
            (true, existing_size + remaining)
        } else {
            (false, response.content_length().unwrap_or(df.size))
        };

        let mut file_downloaded: u64 = if resumed { existing_size } else { 0 };

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

        // Emit initial progress
        emit_multi_progress(app, state, &model.id, cumulative_completed + file_downloaded, total_size, 0);

        let mut last_emit_time = std::time::Instant::now();
        let mut last_emit_bytes = cumulative_completed + file_downloaded;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if cancel_flag.load(Ordering::SeqCst) {
                log::info!("Download cancelled for {} (file: {})", model.id, df.filename);
                return false;
            }
            match chunk {
                Ok(bytes) => {
                    if file.write_all(&bytes).is_err() {
                        return false;
                    }
                    file_downloaded += bytes.len() as u64;

                    let overall = cumulative_completed + file_downloaded;
                    let now = std::time::Instant::now();
                    let elapsed = now.duration_since(last_emit_time);
                    let is_done = file_downloaded >= file_total;
                    if elapsed >= std::time::Duration::from_millis(250) || is_done {
                        let speed = if elapsed.as_secs_f64() > 0.0 {
                            ((overall - last_emit_bytes) as f64 / elapsed.as_secs_f64()) as u64
                        } else { 0 };
                        emit_multi_progress(app, state, &model.id, overall, total_size, speed);
                        last_emit_time = now;
                        last_emit_bytes = overall;
                    }
                }
                Err(e) => {
                    log::error!("Download stream error for {}: {}", df.filename, e);
                    return false;
                }
            }
        }

        // Move partial to final
        if let Err(e) = fs::rename(&tmp_path, &dest_path) {
            // Try copy fallback
            match fs::copy(&tmp_path, &dest_path) {
                Ok(_) => { let _ = fs::remove_file(&tmp_path); }
                Err(e2) => {
                    log::error!("Failed to finalize {}: rename={}, copy={}", df.filename, e, e2);
                    let _ = fs::remove_file(&tmp_path);
                    return false;
                }
            }
        }

        cumulative_completed += df.size;
        log::info!("Completed file {}/{}: {}", files.iter().position(|x| x.filename == df.filename).unwrap_or(0) + 1, files.len(), df.filename);
    }

    // Write completion marker
    if let Some(marker) = &model.download_marker {
        let marker_path = model_dir.join(marker);
        if let Err(e) = fs::write(&marker_path, "") {
            log::error!("Failed to write completion marker: {}", e);
            return false;
        }
    }

    // Final 100% progress
    emit_multi_progress(app, state, &model.id, total_size, total_size, 0);
    log::info!("All {} files downloaded for {}", files.len(), model.id);
    true
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
