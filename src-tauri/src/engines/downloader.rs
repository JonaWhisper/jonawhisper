use super::{ASRModel, DownloadType};
use crate::state::AppState;
use futures_util::StreamExt;
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
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
            download_single_file(&app, &state, &model, false).await
        }
        DownloadType::ZipArchive => {
            download_single_file(&app, &state, &model, true).await
        }
        DownloadType::HuggingFaceRepo => {
            download_hf_repo(&app, state.clone(), &model).await
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

    let client = &*DOWNLOAD_CLIENT;

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

/// Progress reporter that emits Tauri events for HF repo downloads.
struct HfProgress {
    app: AppHandle,
    model_id: String,
    state: Arc<AppState>,
    /// Total number of files to download.
    total_files: usize,
    /// Number of files fully downloaded so far.
    completed_files: usize,
    /// Size of the current file being downloaded.
    current_file_size: usize,
    /// Bytes downloaded for the current file.
    current_downloaded: usize,
}

impl hf_hub::api::Progress for HfProgress {
    fn init(&mut self, size: usize, filename: &str) {
        self.current_file_size = size;
        self.current_downloaded = 0;
        log::info!("Downloading {} ({} bytes)", filename, size);
    }

    fn update(&mut self, size: usize) {
        self.current_downloaded += size;
        let file_progress = if self.current_file_size > 0 {
            self.current_downloaded as f64 / self.current_file_size as f64
        } else {
            0.0
        };
        let progress = if self.total_files > 0 {
            (self.completed_files as f64 + file_progress) / self.total_files as f64
        } else {
            0.0
        };
        self.state.download.lock().unwrap().progress = progress;
        let _ = self.app.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
            "model_id": self.model_id,
            "progress": progress,
        }));
    }

    fn finish(&mut self) {
        self.completed_files += 1;
    }
}

async fn download_hf_repo(
    app: &AppHandle,
    state: Arc<AppState>,
    model: &ASRModel,
) -> bool {
    let app = app.clone();
    let model_id = model.id.clone();
    let repo_id = model.url.clone();

    // hf-hub uses sync API (ureq), run in blocking thread
    tokio::task::spawn_blocking(move || {
        let api = match hf_hub::api::sync::Api::new() {
            Ok(a) => a,
            Err(e) => {
                log::error!("Failed to create HF API: {}", e);
                return false;
            }
        };

        let repo = api.model(repo_id.clone());

        // List files in the repo
        let info = match repo.info() {
            Ok(i) => i,
            Err(e) => {
                log::error!("Failed to get repo info for {}: {}", repo_id, e);
                return false;
            }
        };

        let filenames: Vec<String> = info.siblings.iter()
            .map(|s| s.rfilename.clone())
            .collect();

        log::info!("HF repo {} has {} files", repo_id, filenames.len());

        let total_files = filenames.len();
        let mut completed = 0usize;

        for filename in &filenames {
            let progress = HfProgress {
                app: app.clone(),
                model_id: model_id.clone(),
                state: state.clone(),
                total_files,
                completed_files: completed,
                current_file_size: 0,
                current_downloaded: 0,
            };

            match repo.download_with_progress(filename, progress) {
                Ok(_path) => {
                    completed += 1;
                    log::info!("Downloaded {}/{}: {}", completed, total_files, filename);
                }
                Err(e) => {
                    log::error!("Failed to download {}: {}", filename, e);
                    return false;
                }
            }
        }

        // Emit final 100% progress
        state.download.lock().unwrap().progress = 1.0;
        let _ = app.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
            "model_id": model_id,
            "progress": 1.0,
        }));

        true
    })
    .await
    .unwrap_or(false)
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
                            let _ = app_clone.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
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
        let _ = app.emit(crate::events::DOWNLOAD_PROGRESS, serde_json::json!({
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
