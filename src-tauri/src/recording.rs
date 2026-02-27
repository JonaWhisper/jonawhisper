use crate::audio;
use crate::platform::hotkey;
use crate::platform::paste;
use crate::platform;
use crate::post_processor;
use crate::state::AppState;
use crate::transcriber;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

// -- Timing constants --

const SHORT_TAP_MS: u64 = 300;
const DOUBLE_TAP_MS: u64 = 500;
const ERROR_DISPLAY_MS: u64 = 800;
const SPECTRUM_INTERVAL_MS: u64 = 33;
const ORPHAN_CLEANUP_SECS: u64 = 300;
const CLIPBOARD_RESTORE_DELAY_MS: u64 = 300;

// -- Audio commands for the dedicated audio thread --

pub enum AudioCmd {
    StartRecording { device_uid: Option<String> },
    StopRecording,
    GetSpectrum,
}

pub enum AudioReply {
    Started,
    Stopped { path: Option<std::path::PathBuf> },
    #[allow(dead_code)] // value read via shared spectrum_data, not from the reply
    Spectrum(Vec<f32>),
}

// -- Recording state (Send-safe, does not hold AudioRecorder) --

pub struct RecordingState {
    key_down_time: Option<Instant>,
    last_short_tap_time: Option<Instant>,
    audio_tx: std::sync::mpsc::Sender<AudioCmd>,
    audio_rx: std::sync::mpsc::Receiver<AudioReply>,
}

// SAFETY: The channel endpoints are Send+Sync safe
unsafe impl Send for RecordingState {}
unsafe impl Sync for RecordingState {}

// -- Recording lifecycle --

pub fn start_recording(app: &AppHandle, state: &Arc<AppState>, rec: &mut RecordingState) {
    if *state.is_recording.lock().unwrap() {
        return;
    }
    *state.is_recording.lock().unwrap() = true;
    *state.transcription_cancelled.lock().unwrap() = false;
    rec.key_down_time = Some(Instant::now());

    let device_uid = state.selected_input_device_uid.lock().unwrap().clone();
    let _ = rec.audio_tx.send(AudioCmd::StartRecording { device_uid });
    let _ = rec.audio_rx.recv();

    platform::play_sound("Tink");
    crate::tray::open_pill_window(app);
    crate::tray::set_tray_state(app, "recording");
    let _ = app.emit("pill-mode", "recording");
    let _ = app.emit("recording-started", ());
}

pub fn stop_recording_and_enqueue(
    app: &AppHandle,
    state: &Arc<AppState>,
    rec: &mut RecordingState,
) {
    if !*state.is_recording.lock().unwrap() {
        return;
    }
    *state.is_recording.lock().unwrap() = false;

    let _ = rec.audio_tx.send(AudioCmd::StopRecording);
    let audio_path = match rec.audio_rx.recv() {
        Ok(AudioReply::Stopped { path }) => path,
        _ => None,
    };

    let held_duration = rec.key_down_time.map(|t| t.elapsed());
    rec.key_down_time = None;
    let is_short_tap = held_duration
        .map(|d| d < Duration::from_millis(SHORT_TAP_MS))
        .unwrap_or(false);

    if is_short_tap {
        handle_short_tap(app, state, rec, audio_path);
        return;
    }

    rec.last_short_tap_time = None;

    let audio_path = match audio_path {
        Some(p) => p,
        None => {
            log::warn!("No audio file produced, closing pill");
            crate::tray::close_pill_window(app);
            let _ = app.emit("recording-stopped", ());
            return;
        }
    };

    platform::play_sound("Pop");

    let count = state.enqueue(audio_path);
    let _ = app.emit(
        "recording-stopped",
        serde_json::json!({ "queue_count": count }),
    );
    let _ = app.emit("pill-mode", "transcribing");
    crate::tray::set_tray_state(app, "transcribing");

    // Save clipboard once before the paste batch starts
    {
        let mut saved = state.saved_clipboard.lock().unwrap();
        if saved.is_none() {
            *saved = paste::save_clipboard(app);
            log::info!("Clipboard saved before paste batch");
        }
    }

    let app_clone = app.clone();
    let state_clone = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        process_next_in_queue(&app_clone, &state_clone).await;
    });
}

fn handle_short_tap(
    app: &AppHandle,
    state: &Arc<AppState>,
    rec: &mut RecordingState,
    audio_path: Option<std::path::PathBuf>,
) {
    if let Some(ref path) = audio_path {
        let _ = std::fs::remove_file(path);
    }

    // Double-tap: cancel transcription
    if let Some(last) = rec.last_short_tap_time {
        if last.elapsed() < Duration::from_millis(DOUBLE_TAP_MS) {
            rec.last_short_tap_time = None;
            cancel_transcription(app, state);
            return;
        }
    }
    rec.last_short_tap_time = Some(Instant::now());

    let is_transcribing = *state.is_transcribing.lock().unwrap();
    let queue_empty = state.transcription_queue.lock().unwrap().is_empty();
    if !is_transcribing && queue_empty {
        crate::tray::close_pill_window(app);
    } else {
        let _ = app.emit("pill-mode", "transcribing");
        crate::tray::set_tray_state(app, "transcribing");
    }
    let _ = app.emit("recording-stopped", ());
}

// -- Transcription queue processing --

pub async fn process_next_in_queue(app: &AppHandle, state: &Arc<AppState>) {
    if *state.is_transcribing.lock().unwrap() {
        return;
    }
    if state.transcription_queue.lock().unwrap().is_empty() {
        return;
    }

    *state.is_transcribing.lock().unwrap() = true;
    let audio_path = match state.dequeue() {
        Some(p) => p,
        None => {
            *state.is_transcribing.lock().unwrap() = false;
            return;
        }
    };

    let _ = app.emit(
        "transcription-started",
        serde_json::json!({ "queue_count": state.queue_count() }),
    );

    let had_error = run_transcription(app, state, &audio_path).await;
    let _ = std::fs::remove_file(&audio_path);
    *state.is_transcribing.lock().unwrap() = false;

    if had_error {
        restore_saved_clipboard(app, state);
        show_error_then_close(app);
        return;
    }

    // Continue processing queue if not cancelled
    if !*state.transcription_cancelled.lock().unwrap()
        && !state.transcription_queue.lock().unwrap().is_empty()
    {
        Box::pin(process_next_in_queue(app, &Arc::clone(state))).await;
        return;
    }

    // Queue empty → wait for paste to be processed, then restore clipboard
    tokio::time::sleep(Duration::from_millis(CLIPBOARD_RESTORE_DELAY_MS)).await;
    restore_saved_clipboard(app, state);
    if !*state.is_recording.lock().unwrap() {
        crate::tray::close_pill_window(app);
    }
}

async fn run_transcription(
    app: &AppHandle,
    state: &Arc<AppState>,
    audio_path: &std::path::Path,
) -> bool {
    let state_clone = Arc::clone(state);
    let path = audio_path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        transcriber::transcribe(&state_clone, &path)
    })
    .await;

    match result {
        Ok(Ok(text)) => {
            if *state.transcription_cancelled.lock().unwrap() {
                log::info!("Transcription result discarded (cancelled)");
                return false;
            }
            handle_transcription_result(app, state, &text);
            false
        }
        Ok(Err(e)) => {
            log::error!("Transcription error: {}", e);
            platform::play_sound("Basso");
            let _ = app.emit(
                "transcription-error",
                serde_json::json!({ "error": e.to_string() }),
            );
            true
        }
        Err(e) => {
            log::error!("Transcription task panicked: {}", e);
            platform::play_sound("Basso");
            let _ = app.emit(
                "transcription-error",
                serde_json::json!({ "error": "Internal error" }),
            );
            true
        }
    }
}

fn handle_transcription_result(app: &AppHandle, state: &Arc<AppState>, text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        platform::play_sound("Basso");
        let _ = app.emit("transcription-complete", serde_json::json!({ "text": "" }));
        return;
    }

    let processed = if *state.post_processing_enabled.lock().unwrap() {
        let lang = state.selected_language.lock().unwrap().clone();
        let opts = post_processor::PostProcessOptions {
            hallucination_filter: *state.hallucination_filter_enabled.lock().unwrap(),
        };
        post_processor::process(trimmed, &lang, &opts)
    } else {
        trimmed.to_string()
    };

    paste::paste_text(app, &processed);
    state.add_history(processed.clone());
    platform::play_sound("Glass");

    let _ = app.emit(
        "transcription-complete",
        serde_json::json!({ "text": processed }),
    );
}

// -- Cleanup helpers --

fn show_error_then_close(app: &AppHandle) {
    let _ = app.emit("pill-mode", "error");
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ERROR_DISPLAY_MS)).await;
        crate::tray::close_pill_window(&app_clone);
    });
}

fn restore_saved_clipboard(app: &AppHandle, state: &Arc<AppState>) {
    let saved = state.saved_clipboard.lock().unwrap().take();
    if saved.is_some() {
        paste::restore_clipboard(app, saved);
        log::info!("Clipboard restored after paste batch");
    }
}

fn cancel_transcription(app: &AppHandle, state: &Arc<AppState>) {
    while let Some(path) = state.dequeue() {
        let _ = std::fs::remove_file(&path);
    }
    *state.transcription_cancelled.lock().unwrap() = true;
    restore_saved_clipboard(app, state);
    platform::play_sound("Funk");
    let _ = app.emit("transcription-cancelled", ());
    show_error_then_close(app);
}

pub fn cleanup_orphan_audio_files() {
    let tmp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        let cutoff = std::time::SystemTime::now() - Duration::from_secs(ORPHAN_CLEANUP_SECS);
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("whisper_dictate_") && name.ends_with(".wav") {
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

// -- Thread spawning --

/// Return type for spawn_audio_thread: (cmd_tx, spectrum_data, reply_rx, stream_error).
type AudioThreadHandles = (
    std::sync::mpsc::Sender<AudioCmd>,
    Arc<std::sync::Mutex<Vec<f32>>>,
    std::sync::mpsc::Receiver<AudioReply>,
    Arc<AtomicBool>,
);

/// Spawns the dedicated audio thread (cpal::Stream is not Send).
pub fn spawn_audio_thread() -> AudioThreadHandles {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<AudioCmd>();
    let (reply_tx, reply_rx) = std::sync::mpsc::channel::<AudioReply>();
    let spectrum_data = Arc::new(std::sync::Mutex::new(vec![0.0f32; 12]));
    let spectrum_clone = spectrum_data.clone();

    let stream_error = Arc::new(AtomicBool::new(false));
    let stream_error_clone = Arc::clone(&stream_error);

    std::thread::spawn(move || {
        let mut recorder = audio::AudioRecorder::new(stream_error_clone);
        loop {
            match cmd_rx.recv() {
                Ok(AudioCmd::StartRecording { device_uid }) => {
                    recorder.start_recording(device_uid.as_deref());
                    let _ = reply_tx.send(AudioReply::Started);
                }
                Ok(AudioCmd::StopRecording) => {
                    let path = recorder.stop_recording();
                    let _ = reply_tx.send(AudioReply::Stopped { path });
                }
                Ok(AudioCmd::GetSpectrum) => {
                    let s = recorder.get_spectrum();
                    *spectrum_clone.lock().unwrap() = s.clone();
                }
                Err(_) => break,
            }
        }
    });

    (cmd_tx, spectrum_data, reply_rx, stream_error)
}

pub fn spawn_hotkey_handler(
    hotkey_rx: std::sync::mpsc::Receiver<hotkey::HotkeyEvent>,
    app: AppHandle,
    state: Arc<AppState>,
    rec_state: Arc<std::sync::Mutex<RecordingState>>,
) {
    std::thread::spawn(move || loop {
        match hotkey_rx.recv() {
            Ok(hotkey::HotkeyEvent::KeyDown) => {
                let mut rec = rec_state.lock().unwrap();
                start_recording(&app, &state, &mut rec);
            }
            Ok(hotkey::HotkeyEvent::KeyUp) => {
                let mut rec = rec_state.lock().unwrap();
                stop_recording_and_enqueue(&app, &state, &mut rec);
            }
            Ok(hotkey::HotkeyEvent::CancelPressed) => {
                let is_transcribing = *state.is_transcribing.lock().unwrap();
                let has_queue = !state.transcription_queue.lock().unwrap().is_empty();
                if is_transcribing || has_queue {
                    log::info!("Cancel shortcut pressed, cancelling transcription");
                    cancel_transcription(&app, &state);
                }
            }
            Err(_) => break,
        }
    });
}

/// Spawns the spectrum emission timer (30fps) and monitors stream errors.
pub fn spawn_spectrum_emitter(
    app: AppHandle,
    state: Arc<AppState>,
    cmd_tx: std::sync::mpsc::Sender<AudioCmd>,
    spectrum_data: Arc<std::sync::Mutex<Vec<f32>>>,
    stream_error: Arc<AtomicBool>,
) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(SPECTRUM_INTERVAL_MS));
        if !*state.is_recording.lock().unwrap() {
            continue;
        }

        // Detect audio stream error (e.g. device disconnected)
        if stream_error.load(Ordering::SeqCst) {
            log::warn!("Audio stream error detected (device disconnected?), forcing stop");
            *state.is_recording.lock().unwrap() = false;
            stream_error.store(false, Ordering::SeqCst);

            platform::play_sound("Basso");
            let _ = app.emit("recording-stopped", ());
            restore_saved_clipboard(&app, &state);
            show_error_then_close(&app);
            continue;
        }

        let spectrum = spectrum_data.lock().unwrap().clone();
        let _ = cmd_tx.send(AudioCmd::GetSpectrum);
        let _ = app.emit("spectrum-data", spectrum);
    });
}

pub fn new_recording_state(
    cmd_tx: std::sync::mpsc::Sender<AudioCmd>,
    reply_rx: std::sync::mpsc::Receiver<AudioReply>,
) -> RecordingState {
    RecordingState {
        key_down_time: None,
        last_short_tap_time: None,
        audio_tx: cmd_tx,
        audio_rx: reply_rx,
    }
}
