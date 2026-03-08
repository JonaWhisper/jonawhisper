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

static DOWNLOAD_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .expect("failed to build download client")
});

/// Blocking client that does NOT follow redirects (for x-linked-etag on HuggingFace).
static ETAG_CLIENT_NO_REDIRECT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("failed to build no-redirect client")
});

/// Blocking client that follows redirects (for standard etag).
static ETAG_CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("failed to build etag client")
});

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
            let mut all_complete = true;
            for f in files {
                let file_path = model_dir.join(&f.filename);
                if file_path.exists() {
                    completed_bytes += f.size;
                } else {
                    all_complete = false;
                    // Check for partial
                    let p = multi_file_partial_path(model, &f.filename);
                    completed_bytes += fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                }
            }
            // All files present but marker missing — write it now (crash recovery)
            if all_complete {
                if let Some(marker) = &model.download_marker {
                    let marker_path = model_dir.join(marker);
                    if !marker_path.exists() {
                        let _ = fs::write(&marker_path, "");
                        log::info!("Recovered missing completion marker for {}", model.id);
                    }
                }
                return None;
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

/// Fetch the ETag for a URL using HEAD requests.
///
/// Strategy:
/// 1. HEAD without following redirects → check `x-linked-etag` (HuggingFace content-addressed)
/// 2. If absent, HEAD with redirects → check standard `etag`
/// 3. If neither → None
fn fetch_etag(url: &str) -> Option<String> {
    // Step 1: no-redirect HEAD for x-linked-etag (HuggingFace)
    if let Ok(resp) = ETAG_CLIENT_NO_REDIRECT.head(url).send() {
        if let Some(val) = resp.headers().get("x-linked-etag") {
            if let Ok(s) = val.to_str() {
                return Some(s.to_string());
            }
        }
    }

    // Step 2: follow redirects, check standard etag
    if let Ok(resp) = ETAG_CLIENT.head(url).send() {
        if let Some(val) = resp.headers().get("etag") {
            if let Ok(s) = val.to_str() {
                return Some(s.to_string());
            }
        }
    }

    None
}

/// Write a `version.json` alongside the model after successful download.
/// Includes URL, ETag (from HTTP HEAD), and SHA256 (computed locally) for each file.
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
            let mut file_entries = serde_json::Map::new();
            for f in files {
                let fpath = model_path.join(&f.filename);
                let sha256 = sha256_file(&fpath);
                let etag = fetch_etag(&f.url);
                let mut entry = serde_json::Map::new();
                entry.insert("url".into(), serde_json::Value::String(f.url.clone()));
                if let Some(e) = etag {
                    entry.insert("etag".into(), serde_json::Value::String(e));
                }
                if let Some(h) = sha256 {
                    entry.insert("sha256".into(), serde_json::Value::String(h));
                }
                file_entries.insert(f.filename.clone(), serde_json::Value::Object(entry));
            }
            serde_json::json!({
                "model_id": model.id,
                "files": file_entries,
                "downloaded_at": now,
            })
        }
        _ => {
            let sha256 = sha256_file(&model_path).unwrap_or_default();
            let etag = fetch_etag(&model.url);
            let mut json = serde_json::json!({
                "model_id": model.id,
                "url": model.url,
                "sha256": sha256,
                "downloaded_at": now,
            });
            if let Some(e) = etag {
                json["etag"] = serde_json::Value::String(e);
            }
            json
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

/// Emit download progress event and update active download state.
fn emit_progress(
    app: &AppHandle, download_state: &Mutex<DownloadState>, model_id: &str,
    downloaded: u64, total: u64, speed: u64,
) {
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
}

/// Download a single URL to `tmp_path`, with Range resume and progress reporting.
/// Returns the number of bytes downloaded for this file, or None on failure/cancel.
async fn download_one_file(
    url: &str,
    tmp_path: &Path,
    cancel_flag: &AtomicBool,
    progress_cb: &mut (dyn FnMut(u64, u64) + Send),
) -> Option<u64> {
    let client = &*DOWNLOAD_CLIENT;
    let existing_size = fs::metadata(tmp_path).map(|m| m.len()).unwrap_or(0);

    let mut request = client.get(url);
    if existing_size > 0 {
        log::info!("Resuming from {} bytes: {}", existing_size, url);
        request = request.header("Range", format!("bytes={}-", existing_size));
    }

    let response = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Download failed: {}", e);
            return None;
        }
    };

    let status = response.status();

    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        log::warn!("Server returned 416, deleting partial and retrying: {}", url);
        let _ = fs::remove_file(tmp_path);
        return Box::pin(download_one_file(url, tmp_path, cancel_flag, progress_cb)).await;
    }

    let (resumed, total_size) = if status == reqwest::StatusCode::PARTIAL_CONTENT {
        let remaining = response.content_length().unwrap_or(0);
        (true, existing_size + remaining)
    } else {
        (false, response.content_length().unwrap_or(0))
    };

    let mut downloaded: u64 = if resumed { existing_size } else { 0 };

    let mut file = if resumed {
        match fs::OpenOptions::new().append(true).open(tmp_path) {
            Ok(f) => f,
            Err(e) => { log::error!("Failed to open partial for append: {}", e); return None; }
        }
    } else {
        match fs::File::create(tmp_path) {
            Ok(f) => f,
            Err(e) => { log::error!("Failed to create temp file: {}", e); return None; }
        }
    };

    if resumed && total_size > 0 {
        progress_cb(downloaded, total_size);
    }

    let mut last_emit_time = std::time::Instant::now();

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancel_flag.load(Ordering::SeqCst) {
            log::info!("Download cancelled");
            return None;
        }
        match chunk {
            Ok(bytes) => {
                if file.write_all(&bytes).is_err() { return None; }
                downloaded += bytes.len() as u64;
                if total_size > 0 {
                    let now = std::time::Instant::now();
                    let elapsed = now.duration_since(last_emit_time);
                    let is_done = downloaded >= total_size;
                    if elapsed >= std::time::Duration::from_millis(250) || is_done {
                        progress_cb(downloaded, total_size);
                        last_emit_time = now;
                    }
                }
            }
            Err(e) => {
                log::error!("Download stream error: {}", e);
                return None;
            }
        }
    }

    Some(downloaded)
}

/// Move tmp_path to dest_path with rename + copy fallback.
fn finalize_file(tmp_path: &Path, dest_path: &Path) -> bool {
    if dest_path.exists() { let _ = fs::remove_file(dest_path); }
    match fs::rename(tmp_path, dest_path) {
        Ok(()) => true,
        Err(_) => match fs::copy(tmp_path, dest_path) {
            Ok(_) => { let _ = fs::remove_file(tmp_path); true }
            Err(e) => { log::error!("Failed to move file: {}", e); let _ = fs::remove_file(tmp_path); false }
        }
    }
}

async fn download_single_file(
    app: &AppHandle,
    download_state: &Mutex<DownloadState>,
    model: &ASRModel,
    cancel_flag: &std::sync::atomic::AtomicBool,
) -> bool {
    let storage_dir = shellexpand::tilde(&model.storage_dir).to_string();
    let _ = fs::create_dir_all(&storage_dir);

    let dest_path = model.local_path();
    let tmp_path = partial_path(model);
    let model_id = model.id.clone();

    let mut last_bytes: u64 = 0;
    let mut last_time = std::time::Instant::now();
    let mut progress_cb = |downloaded: u64, total: u64| {
        let now = std::time::Instant::now();
        let dt = now.duration_since(last_time).as_secs_f64();
        let speed = if dt > 0.0 { ((downloaded - last_bytes) as f64 / dt) as u64 } else { 0 };
        last_bytes = downloaded;
        last_time = now;
        emit_progress(app, download_state, &model_id, downloaded, total, speed);
    };

    let downloaded = download_one_file(&model.url, &tmp_path, cancel_flag, &mut progress_cb).await;

    match downloaded {
        Some(bytes) => {
            if model.size > 0 && bytes < model.size / 2 {
                log::error!("Downloaded size ({}) suspiciously small for {} (expected ~{})", bytes, model.id, model.size);
                let _ = fs::remove_file(&tmp_path);
                return false;
            }
            finalize_file(&tmp_path, &dest_path)
        }
        None => false,
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
    let mut cumulative_completed: u64 = 0;

    for df in files {
        let dest_path = model_dir.join(&df.filename);

        if dest_path.exists() {
            cumulative_completed += df.size;
            emit_progress(app, download_state, &model.id, cumulative_completed, total_size, 0);
            continue;
        }

        let tmp_path = multi_file_partial_path(model, &df.filename);
        let base = cumulative_completed;
        let model_id = model.id.clone();

        let mut last_bytes: u64 = 0;
        let mut last_time = std::time::Instant::now();
        let mut progress_cb = |file_downloaded: u64, _file_total: u64| {
            let now = std::time::Instant::now();
            let dt = now.duration_since(last_time).as_secs_f64();
            let speed = if dt > 0.0 { ((file_downloaded - last_bytes) as f64 / dt) as u64 } else { 0 };
            last_bytes = file_downloaded;
            last_time = now;
            emit_progress(app, download_state, &model_id, base + file_downloaded, total_size, speed);
        };

        let result = download_one_file(&df.url, &tmp_path, cancel_flag, &mut progress_cb).await;

        match result {
            Some(_) => {
                if !finalize_file(&tmp_path, &dest_path) { return false; }
            }
            None => return false,
        }

        cumulative_completed += df.size;
        log::info!("Completed file {}/{}: {}", files.iter().position(|x| x.filename == df.filename).unwrap_or(0) + 1, files.len(), df.filename);
    }

    if let Some(marker) = &model.download_marker {
        let marker_path = model_dir.join(marker);
        if let Err(e) = fs::write(&marker_path, "") {
            log::error!("Failed to write completion marker: {}", e);
            return false;
        }
    }

    emit_progress(app, download_state, &model.id, total_size, total_size, 0);
    log::info!("All {} files downloaded for {}", files.len(), model.id);
    true
}

// -- Update detection --

/// Result of comparing local version.json ETags against remote.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    /// Remote ETags match local — model is current.
    UpToDate,
    /// At least one ETag differs — a newer version is available.
    UpdateAvailable,
    /// No version.json, no stored ETags, or network error — cannot determine.
    Unknown,
}

/// Check whether a downloaded model has an update available by comparing
/// the ETags stored in its local `version.json` against the current remote ETags.
pub fn check_model_update(model: &ASRModel) -> UpdateStatus {
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

    // Collect (url, stored_etag) pairs from version.json
    let mut url_etags: Vec<(String, String)> = Vec::new();

    if let Some(files) = local.get("files").and_then(|v| v.as_object()) {
        // Multi-file model: only check model files (skip config.json, tokenizer.json etc.)
        for (filename, entry) in files {
            if filename == "config.json" || filename == "tokenizer.json" {
                continue;
            }
            let url = match entry.get("url").and_then(|v| v.as_str()) {
                Some(u) => u.to_string(),
                None => continue,
            };
            let etag = match entry.get("etag").and_then(|v| v.as_str()) {
                Some(e) => e.to_string(),
                None => continue, // No stored ETag for this file — skip
            };
            url_etags.push((url, etag));
        }
    } else if let Some(url) = local.get("url").and_then(|v| v.as_str()) {
        // Single-file model
        let etag = match local.get("etag").and_then(|v| v.as_str()) {
            Some(e) => e.to_string(),
            None => return UpdateStatus::Unknown,
        };
        url_etags.push((url.to_string(), etag));
    }

    if url_etags.is_empty() {
        return UpdateStatus::Unknown;
    }

    // Compare each stored ETag against the remote
    for (url, stored_etag) in &url_etags {
        match fetch_etag(url) {
            Some(remote_etag) => {
                if remote_etag != *stored_etag {
                    log::info!("ETag changed for {}: {} -> {}", model.id, stored_etag, remote_etag);
                    return UpdateStatus::UpdateAvailable;
                }
            }
            None => {
                // Network error or no ETag from server — cannot determine
                return UpdateStatus::Unknown;
            }
        }
    }

    UpdateStatus::UpToDate
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_status_equality() {
        assert_eq!(UpdateStatus::UpToDate, UpdateStatus::UpToDate);
        assert_eq!(UpdateStatus::UpdateAvailable, UpdateStatus::UpdateAvailable);
        assert_eq!(UpdateStatus::Unknown, UpdateStatus::Unknown);
        assert_ne!(UpdateStatus::UpToDate, UpdateStatus::UpdateAvailable);
    }

    #[test]
    fn update_status_serde() {
        let json = serde_json::to_string(&UpdateStatus::UpToDate).unwrap();
        assert!(json.contains("up_to_date"));

        let json = serde_json::to_string(&UpdateStatus::UpdateAvailable).unwrap();
        assert!(json.contains("update_available"));

        let json = serde_json::to_string(&UpdateStatus::Unknown).unwrap();
        assert!(json.contains("unknown"));
    }

    #[test]
    fn partial_path_deterministic() {
        let model = ASRModel {
            id: "whisper:tiny".to_string(),
            storage_dir: "/tmp/test_storage".to_string(),
            filename: "model.bin".to_string(),
            ..Default::default()
        };
        let p1 = partial_path(&model);
        let p2 = partial_path(&model);
        assert_eq!(p1, p2);
        assert!(p1.to_string_lossy().contains(".partial"));
    }

    #[test]
    fn partial_path_sanitizes_id() {
        let model = ASRModel {
            id: "engine:model/variant".to_string(),
            storage_dir: "/tmp/test".to_string(),
            filename: "m.bin".to_string(),
            ..Default::default()
        };
        let p = partial_path(&model);
        let name = p.file_name().unwrap().to_string_lossy();
        // Colons and slashes should be replaced with underscores
        assert!(!name.contains(':'));
        assert!(!name.contains('/'));
    }

    #[test]
    fn partial_progress_no_partial_file() {
        let model = ASRModel {
            id: "test:nonexistent".to_string(),
            storage_dir: "/tmp/jona_test_no_partial".to_string(),
            filename: "model.bin".to_string(),
            size: 1000,
            ..Default::default()
        };
        assert!(partial_progress(&model).is_none());
    }

    #[test]
    fn partial_progress_zero_size_model() {
        let model = ASRModel {
            id: "test:zero".to_string(),
            size: 0,
            ..Default::default()
        };
        assert!(partial_progress(&model).is_none());
    }

    #[test]
    fn partial_progress_multifile_all_complete_recovers_marker() {
        let dir = std::env::temp_dir().join("jona_test_multifile_recovery");
        let model_dir = dir.join("spellcheck_en");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&model_dir);

        // Write all files but NO .complete marker
        std::fs::write(model_dir.join("freq.txt"), "word 100").unwrap();
        std::fs::write(model_dir.join("bigram.txt"), "a b 50").unwrap();

        let model = ASRModel {
            id: "spellcheck:en".to_string(),
            storage_dir: dir.to_string_lossy().to_string(),
            filename: "spellcheck_en".to_string(),
            size: 1000,
            download_marker: Some(".complete".to_string()),
            download_type: DownloadType::MultiFile {
                files: vec![
                    DownloadFile { url: String::new(), filename: "freq.txt".into(), size: 500 },
                    DownloadFile { url: String::new(), filename: "bigram.txt".into(), size: 500 },
                ],
            },
            ..Default::default()
        };

        // Before fix: would return Some(0.99), model shows as "paused"
        // After fix: returns None, auto-writes .complete marker
        assert!(partial_progress(&model).is_none());
        assert!(model_dir.join(".complete").exists());
        assert!(model.is_downloaded());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_model_not_downloaded() {
        let model = ASRModel {
            storage_dir: "/tmp/jona_test_delete_nonexistent".to_string(),
            filename: "model.bin".to_string(),
            ..Default::default()
        };
        assert!(!delete_model(&model));
    }

    #[test]
    fn delete_model_single_file() {
        let dir = std::env::temp_dir().join("jona_test_delete_single");
        let _ = std::fs::create_dir_all(&dir);
        let file = dir.join("model.bin");
        std::fs::write(&file, b"test data").unwrap();

        let model = ASRModel {
            storage_dir: dir.to_string_lossy().to_string(),
            filename: "model.bin".to_string(),
            ..Default::default()
        };
        assert!(model.is_downloaded());
        assert!(delete_model(&model));
        assert!(!file.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_model_directory() {
        let dir = std::env::temp_dir().join("jona_test_delete_dir");
        let model_dir = dir.join("my_model");
        let _ = std::fs::create_dir_all(&model_dir);
        std::fs::write(model_dir.join(".complete"), "").unwrap();
        std::fs::write(model_dir.join("weights.bin"), b"data").unwrap();

        let model = ASRModel {
            storage_dir: dir.to_string_lossy().to_string(),
            filename: "my_model".to_string(),
            download_marker: Some(".complete".to_string()),
            download_type: DownloadType::MultiFile { files: vec![] },
            ..Default::default()
        };
        assert!(model.is_downloaded());
        assert!(delete_model(&model));
        assert!(!model_dir.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
