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

pub fn start_recording(app: &AppHandle, state: &Arc<AppState>, rec: &mut RecordingState) {
    if *state.is_recording.lock().unwrap() {
        return;
    }
    *state.is_recording.lock().unwrap() = true;
    *state.transcription_cancelled.lock().unwrap() = false;
    rec.key_down_time = Some(Instant::now());

    let device_uid = state.selected_input_device_uid.lock().unwrap().clone();
    let _ = rec.audio_tx.send(AudioCmd::StartRecording { device_uid });
    // Wait for ack
    let _ = rec.audio_rx.recv();

    platform::play_sound("Tink");
    crate::tray::open_pill_window(app);
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
    log::info!("stop_recording: audio_path={:?}", audio_path.as_ref().map(|p| p.display().to_string()));

    // Detect short tap (< 300ms)
    let is_short_tap = rec
        .key_down_time
        .map(|t| t.elapsed() < Duration::from_millis(300))
        .unwrap_or(false);
    rec.key_down_time = None;
    log::info!("stop_recording: is_short_tap={}", is_short_tap);

    if is_short_tap {
        if let Some(ref path) = audio_path {
            let _ = std::fs::remove_file(path);
        }

        if let Some(last) = rec.last_short_tap_time {
            if last.elapsed() < Duration::from_millis(500) {
                rec.last_short_tap_time = None;
                cancel_transcription(app, state);
                return;
            }
        }
        rec.last_short_tap_time = Some(Instant::now());

        let is_transcribing = *state.is_transcribing.lock().unwrap();
        let queue_empty = state.transcription_queue.lock().unwrap().is_empty();
        if !is_transcribing && queue_empty {
            log::info!("stop_recording: short tap, nothing in progress → closing pill");
            crate::tray::close_pill_window(app);
        } else {
            log::info!("stop_recording: short tap, transcription in progress → keeping pill");
            let _ = app.emit("pill-mode", "transcribing");
        }
        let _ = app.emit("recording-stopped", ());
        return;
    }

    rec.last_short_tap_time = None;

    let audio_path = match audio_path {
        Some(p) => p,
        None => {
            log::warn!("stop_recording: no audio file produced, closing pill");
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

    log::info!("stop_recording: enqueued, emitting pill-mode=transcribing");
    let _ = app.emit("pill-mode", "transcribing");

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

pub async fn process_next_in_queue(app: &AppHandle, state: &Arc<AppState>) {
    if *state.is_transcribing.lock().unwrap() {
        log::info!("process_next_in_queue: already transcribing, skipping");
        return;
    }
    if state.transcription_queue.lock().unwrap().is_empty() {
        log::info!("process_next_in_queue: queue empty, skipping");
        return;
    }

    log::info!("process_next_in_queue: starting transcription");
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

    let state_clone = Arc::clone(state);
    let audio_path_clone = audio_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        transcriber::transcribe(&state_clone, &audio_path_clone)
    })
    .await;

    let _ = std::fs::remove_file(&audio_path);

    let had_error;
    match result {
        Ok(Ok(text)) => {
            had_error = false;
            if *state.transcription_cancelled.lock().unwrap() {
                log::info!("Transcription result discarded (cancelled)");
            } else {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let processed = if *state.post_processing_enabled.lock().unwrap() {
                        let lang = state.selected_language.lock().unwrap().clone();
                        post_processor::process(trimmed, &lang)
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
                } else {
                    platform::play_sound("Basso");
                    let _ = app.emit("transcription-complete", serde_json::json!({ "text": "" }));
                }
            }
        }
        Ok(Err(e)) => {
            had_error = true;
            log::error!("Transcription error: {}", e);
            platform::play_sound("Basso");
            let _ = app.emit(
                "transcription-error",
                serde_json::json!({ "error": e.to_string() }),
            );
        }
        Err(e) => {
            had_error = true;
            log::error!("Transcription task panicked: {}", e);
            platform::play_sound("Basso");
            let _ = app.emit(
                "transcription-error",
                serde_json::json!({ "error": "Internal error" }),
            );
        }
    }

    *state.is_transcribing.lock().unwrap() = false;

    log::info!("process_next_in_queue: transcription done, had_error={}", had_error);

    // Error → show error 800ms then close
    if had_error {
        restore_saved_clipboard(app, state);
        let _ = app.emit("pill-mode", "error");
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(800)).await;
            crate::tray::close_pill_window(&app_clone);
        });
        return;
    }

    // Success → continue queue or close
    if !*state.transcription_cancelled.lock().unwrap()
        && !state.transcription_queue.lock().unwrap().is_empty()
    {
        let app_clone = app.clone();
        let state_clone = Arc::clone(state);
        Box::pin(process_next_in_queue(&app_clone, &state_clone)).await;
        return;
    }

    // Queue empty → restore clipboard and close pill
    restore_saved_clipboard(app, state);
    if !*state.is_recording.lock().unwrap() {
        crate::tray::close_pill_window(app);
    }
}

/// Restore the clipboard content that was saved before the paste batch.
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
    let _ = app.emit("pill-mode", "error");
    let _ = app.emit("transcription-cancelled", ());
    // Close after 800ms error animation
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        crate::tray::close_pill_window(&app_clone);
    });
}

pub fn cleanup_orphan_audio_files() {
    let tmp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        let cutoff = std::time::SystemTime::now() - Duration::from_secs(300);
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
                    // Don't send reply — spectrum data is read via shared spectrum_data
                }
                Err(_) => break,
            }
        }
    });

    (cmd_tx, spectrum_data, reply_rx, stream_error)
}

/// Spawns the hotkey event processing thread.
pub fn spawn_hotkey_handler(
    hotkey_rx: std::sync::mpsc::Receiver<hotkey::HotkeyEvent>,
    app: AppHandle,
    state: Arc<AppState>,
    rec_state: Arc<std::sync::Mutex<RecordingState>>,
) {
    std::thread::spawn(move || loop {
        match hotkey_rx.recv() {
            Ok(hotkey::HotkeyEvent::KeyDown) => {
                log::info!("Hotkey KeyDown received");
                let mut rec = rec_state.lock().unwrap();
                start_recording(&app, &state, &mut rec);
            }
            Ok(hotkey::HotkeyEvent::KeyUp) => {
                log::info!("Hotkey KeyUp received");
                let mut rec = rec_state.lock().unwrap();
                stop_recording_and_enqueue(&app, &state, &mut rec);
            }
            Err(_) => break,
        }
    });
}

/// Spawns the spectrum emission timer (30fps).
/// Also monitors the stream_error flag to detect device disconnection.
pub fn spawn_spectrum_emitter(
    app: AppHandle,
    state: Arc<AppState>,
    cmd_tx: std::sync::mpsc::Sender<AudioCmd>,
    spectrum_data: Arc<std::sync::Mutex<Vec<f32>>>,
    stream_error: Arc<AtomicBool>,
) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(33));
        if *state.is_recording.lock().unwrap() {
            // Detect audio stream error (e.g. device disconnected)
            if stream_error.load(Ordering::SeqCst) {
                log::warn!("Audio stream error detected (device disconnected?), forcing stop");
                *state.is_recording.lock().unwrap() = false;
                stream_error.store(false, Ordering::SeqCst);

                // Show error on pill, then close after 800ms
                platform::play_sound("Basso");
                let _ = app.emit("pill-mode", "error");
                let _ = app.emit("recording-stopped", ());

                // Restore clipboard if we saved it
                restore_saved_clipboard(&app, &state);

                let app_clone = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(800));
                    crate::tray::close_pill_window(&app_clone);
                });
                continue;
            }

            let spectrum = spectrum_data.lock().unwrap().clone();
            let _ = cmd_tx.send(AudioCmd::GetSpectrum);
            let _ = app.emit("spectrum-data", spectrum);
        }
    });
}

/// Creates a new RecordingState from audio thread channels.
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
