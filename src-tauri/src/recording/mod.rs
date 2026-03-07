mod lifecycle;
mod pipeline;
mod threads;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Generation counter — incremented on each recording start, used to prevent
/// stale delayed pill closes (from error display) from killing a freshly opened pill.
static PILL_CLOSE_GENERATION: AtomicU64 = AtomicU64::new(0);

// -- Timing constants --

const SHORT_TAP_MS: u64 = 300;
const DOUBLE_TAP_MS: u64 = 500;
const ERROR_DISPLAY_MS: u64 = 800;
const SPECTRUM_INTERVAL_MS: u64 = 33;
const ORPHAN_CLEANUP_SECS: u64 = 300;

// -- Audio commands for the dedicated audio thread --

pub enum AudioCmd {
    StartRecording { device_uid: Option<String> },
    StopRecording,
    GetSpectrum,
    StartMicTest { device_uid: Option<String> },
    StopMicTest,
}

pub enum AudioReply {
    Started,
    Stopped { path: Option<std::path::PathBuf> },
}

// -- Recording state (Send-safe, does not hold AudioRecorder) --

pub struct RecordingState {
    key_down_time: Option<Instant>,
    last_short_tap_time: Option<Instant>,
    audio_tx: crossbeam_channel::Sender<AudioCmd>,
    audio_rx: crossbeam_channel::Receiver<AudioReply>,
}

/// Wrapper around audio command sender for mic test (managed by Tauri).
pub struct MicTestSender(pub crossbeam_channel::Sender<AudioCmd>);

pub fn new_recording_state(
    cmd_tx: crossbeam_channel::Sender<AudioCmd>,
    reply_rx: crossbeam_channel::Receiver<AudioReply>,
) -> RecordingState {
    RecordingState {
        key_down_time: None,
        last_short_tap_time: None,
        audio_tx: cmd_tx,
        audio_rx: reply_rx,
    }
}

fn show_error_then_close(app: &tauri::AppHandle) {
    crate::ui::pill::set_mode(crate::ui::pill::PillMode::Error);
    let gen = PILL_CLOSE_GENERATION.load(Ordering::SeqCst);
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ERROR_DISPLAY_MS)).await;
        // Only close if no new recording started since the error
        if PILL_CLOSE_GENERATION.load(Ordering::SeqCst) == gen {
            crate::ui::tray::close_pill_window(&app_clone);
        }
    });
}

pub use threads::{spawn_audio_thread, spawn_hotkey_handler, spawn_spectrum_emitter};

pub fn cleanup_orphan_audio_files() {
    let tmp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        let cutoff = std::time::SystemTime::now() - Duration::from_secs(ORPHAN_CLEANUP_SECS);
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if (name.starts_with("jona_whisper_") || name.starts_with("whisper_dictate_")) && name.ends_with(".wav") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            let _ = std::fs::remove_file(entry.path());
                            log::info!("Cleaned orphan audio: {}", name);
                        }
                    }
                }
            }
        }
    }
}
