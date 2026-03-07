use super::{ASRModel, DownloadFile, DownloadType};
use jona_types::{ActiveDownload, DownloadState};
use futures_util::StreamExt;
use sha2::{Sha256, Digest};
use std::fs;
use std::io::{Read as _, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::{AppHandle, Emitter};

static DOWNLOAD_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

pub const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";

fn pending_download_path() -> PathBuf {
    jona_types::config_dir().join(".pending-download")
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

/// Compute the SHA256 hash of a file.
fn sha256_file(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

/// Write a `version.json` alongside the model after successful download.
fn write_version_json(model: &ASRModel) {
    let model_path = model.local_path();
    let now = chrono::Utc::now().to_rfc3339();

    let version_dir = if model_path.is_dir() {
        model_path.clone()
    } else {
        match model_path.parent() {
            Some(p) => p.to_path_buf(),
            None => return,
        }
    };

    let version_path = version_dir.join("version.json");

    let json = match &model.download_type {
        DownloadType::MultiFile { files } => {
            let mut file_hashes = serde_json::Map::new();
            for f in files {
                let fpath = model_path.join(&f.filename);
                if let Some(hash) = sha256_file(&fpath) {
                    file_hashes.insert(f.filename.clone(), serde_json::Value::String(hash));
                }
            }
            serde_json::json!({
                "model_id": model.id,
                "files": file_hashes,
                "downloaded_at": now,
            })
        }
        _ => {
            let hash = sha256_file(&model_path).unwrap_or_default();
            serde_json::json!({
                "model_id": model.id,
                "sha256": hash,
                "downloaded_at": now,
            })
        }
    };

    match fs::write(&version_path, serde_json::to_string_pretty(&json).unwrap_or_default()) {
        Ok(()) => log::info!("Wrote version.json for {}", model.id),
        Err(e) => log::warn!("Failed to write version.json for {}: {}", model.id, e),
    }
}

pub async fn download_model(
    app: AppHandle,
    download_state: Arc<Mutex<DownloadState>>,
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
        let mut dl = download_state.lock().unwrap();
        dl.active.insert(model.id.clone(), ActiveDownload {
            progress: initial_progress,
            cancel_requested: cancel_flag.clone(),
            delete_partial: delete_flag.clone(),
        });
    }

    let _ = app.emit(DOWNLOAD_PROGRESS_EVENT, serde_json::json!({
        "model_id": model.id,
        "progress": initial_progress,
    }));

    let success = match &model.download_type {
        DownloadType::RemoteAPI | DownloadType::System => true,
        DownloadType::SingleFile => {
            download_single_file(&app, &download_state, &model, &cancel_flag).await
        }
        DownloadType::MultiFile { files } => {
            download_multi_file(&app, &download_state, &model, files, &cancel_flag).await
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

    if success {
        write_version_json(&model);
    }

    clear_pending_state(&model);
    // Remove this download from the HashMap
    {
        let mut dl = download_state.lock().unwrap();
        dl.active.remove(&model.id);
    }

    success
}

async fn download_single_file(
    app: &AppHandle,
    download_state: &Mutex<DownloadState>,
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
        return Box::pin(download_single_file(app, download_state, model, cancel_flag)).await;
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
    let emit_progress = |app: &AppHandle, download_state: &Mutex<DownloadState>, model_id: &str,
                         downloaded: u64, total: u64, speed: u64| {
        let progress = if total > 0 { downloaded as f64 / total as f64 } else { 0.0 };
        if let Some(entry) = download_state.lock().unwrap().active.get_mut(model_id) {
            entry.progress = progress;
        }
        let _ = app.emit(DOWNLOAD_PROGRESS_EVENT, serde_json::json!({
            "model_id": model_id,
            "progress": progress,
            "downloaded": downloaded,
            "total_size": total,
            "speed": speed,
        }));
    };

    // Emit initial progress if resuming
    if resumed && total_size > 0 {
        emit_progress(app, download_state, &model.id, downloaded, total_size, 0);
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
                        emit_progress(app, download_state, &model.id, downloaded, total_size, speed);
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
    download_state: &Mutex<DownloadState>,
    model: &ASRModel,
    files: &[DownloadFile],
    cancel_flag: &AtomicBool,
) -> bool {
    let model_dir = model.local_path();
    let _ = fs::create_dir_all(&model_dir);

    let total_size: u64 = files.iter().map(|f| f.size).sum();

    // Emit helper for overall multi-file progress
    let emit_multi_progress = |app: &AppHandle, download_state: &Mutex<DownloadState>, model_id: &str,
                                overall_downloaded: u64, total: u64, speed: u64| {
        let progress = if total > 0 { overall_downloaded as f64 / total as f64 } else { 0.0 };
        if let Some(entry) = download_state.lock().unwrap().active.get_mut(model_id) {
            entry.progress = progress;
        }
        let _ = app.emit(DOWNLOAD_PROGRESS_EVENT, serde_json::json!({
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
            emit_multi_progress(app, download_state, &model.id, cumulative_completed, total_size, 0);
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
            return Box::pin(download_multi_file(app, download_state, model, files, cancel_flag)).await;
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
        emit_multi_progress(app, download_state, &model.id, cumulative_completed + file_downloaded, total_size, 0);

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
                        emit_multi_progress(app, download_state, &model.id, overall, total_size, speed);
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
    emit_multi_progress(app, download_state, &model.id, total_size, total_size, 0);
    log::info!("All {} files downloaded for {}", files.len(), model.id);
    true
}

// -- Update detection --

/// Result of comparing local version.json against catalog hashes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    /// Local hashes match catalog — model is current.
    UpToDate,
    /// Catalog hashes differ from local — a newer version is available.
    UpdateAvailable,
    /// No version.json on disk — cannot determine (treated as up-to-date).
    Unknown,
}

/// Check whether a downloaded model has an update available by comparing
/// the hashes in its local `version.json` against the catalog's `sha256`/`file_hashes`.
pub fn check_model_update(model: &ASRModel) -> UpdateStatus {
    // Models without catalog hashes cannot be checked
    let has_catalog_hash = model.sha256.is_some()
        || model.file_hashes.as_ref().is_some_and(|h| !h.is_empty());
    if !has_catalog_hash {
        return UpdateStatus::Unknown;
    }

    let model_path = model.local_path();
    let version_dir = if model_path.is_dir() {
        model_path
    } else {
        match model_path.parent() {
            Some(p) => p.to_path_buf(),
            None => return UpdateStatus::Unknown,
        }
    };

    let version_path = version_dir.join("version.json");
    let content = match fs::read_to_string(&version_path) {
        Ok(c) => c,
        Err(_) => return UpdateStatus::Unknown,
    };

    let local: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return UpdateStatus::Unknown,
    };

    // Single-file model: compare sha256
    if let Some(catalog_hash) = &model.sha256 {
        let local_hash = local.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
        return if local_hash == catalog_hash {
            UpdateStatus::UpToDate
        } else {
            UpdateStatus::UpdateAvailable
        };
    }

    // Multi-file model: compare per-file hashes
    if let Some(catalog_hashes) = &model.file_hashes {
        let local_files = local.get("files").and_then(|v| v.as_object());
        match local_files {
            None => return UpdateStatus::UpdateAvailable,
            Some(local_map) => {
                for (filename, catalog_hash) in catalog_hashes {
                    let local_hash = local_map.get(filename)
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if local_hash != catalog_hash {
                        return UpdateStatus::UpdateAvailable;
                    }
                }
                return UpdateStatus::UpToDate;
            }
        }
    }

    UpdateStatus::Unknown
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
