use super::{ASRModel, DownloadType};
use crate::state::AppState;
use futures_util::StreamExt;
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

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
            download_single_file(&app, &state, &model, false).await
        }
        DownloadType::ZipArchive => {
            download_single_file(&app, &state, &model, true).await
        }
        DownloadType::HuggingFaceRepo => {
            download_with_subprocess(
                &app, &state, &model,
                "/usr/bin/env",
                &["huggingface-cli".to_string(), "download".to_string(), model.url.clone()],
            ).await
        }
        DownloadType::Command { executable, arguments } => {
            download_with_subprocess(
                &app, &state, &model,
                executable,
                arguments,
            ).await
        }
    };

    clear_pending_state(&model);
    {
        let mut dl = state.download.lock().unwrap();
        dl.model_id = None;
        dl.progress = 0.0;
    }

    let _ = app.emit("download-complete", serde_json::json!({
        "model_id": model.id,
        "success": success,
    }));

    success
}

async fn download_single_file(
    app: &AppHandle,
    state: &AppState,
    model: &ASRModel,
    is_zip: bool,
) -> bool {
    let storage_dir = shellexpand::tilde(&model.storage_dir).to_string();
    let _ = fs::create_dir_all(&storage_dir);

    let client = reqwest::Client::new();

    // Try resume for non-zip downloads
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
                    let _ = app.emit("download-progress", serde_json::json!({
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

    if is_zip {
        // Extract zip
        let status = Command::new("/usr/bin/unzip")
            .args(["-o", &tmp_path.to_string_lossy(), "-d", &storage_dir])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = fs::remove_file(&tmp_path);
        match status {
            Ok(s) if s.success() => true,
            _ => {
                log::error!("Failed to extract zip for model: {}", model.id);
                false
            }
        }
    } else {
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
}

async fn download_with_subprocess(
    app: &AppHandle,
    state: &AppState,
    model: &ASRModel,
    executable: &str,
    arguments: &[String],
) -> bool {
    let app_clone = app.clone();
    let model_id = model.id.clone();
    let exe = executable.to_string();
    let args = arguments.to_vec();

    // Run subprocess in blocking thread
    let result = tokio::task::spawn_blocking(move || {
        let mut child = match Command::new(&exe)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                log::error!("Download subprocess failed to launch: {}", e);
                return false;
            }
        };

        // Parse tqdm progress from stderr
        if let Some(stderr) = child.stderr.take() {
            let reader = std::io::BufReader::new(stderr);
            let mut buf = String::new();
            use std::io::Read;
            let mut bytes = reader.bytes();
            let progress_re = Regex::new(r"(\d+)%").unwrap();

            while let Some(Ok(byte)) = bytes.next() {
                let ch = byte as char;
                if ch == '\r' || ch == '\n' {
                    if let Some(caps) = progress_re.captures(&buf) {
                        if let Ok(pct) = caps[1].parse::<f64>() {
                            let progress = pct / 100.0;
                            let _ = app_clone.emit("download-progress", serde_json::json!({
                                "model_id": model_id,
                                "progress": progress,
                            }));
                        }
                    }
                    buf.clear();
                } else {
                    buf.push(ch);
                }
            }
        }

        match child.wait() {
            Ok(status) => status.success(),
            Err(e) => {
                log::error!("Download subprocess wait failed: {}", e);
                false
            }
        }
    })
    .await
    .unwrap_or(false);

    if result {
        state.download.lock().unwrap().progress = 1.0;
        let _ = app.emit("download-progress", serde_json::json!({
            "model_id": model.id,
            "progress": 1.0,
        }));
    }

    result
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
