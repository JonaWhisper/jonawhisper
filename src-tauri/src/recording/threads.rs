use super::{AudioCmd, AudioReply, RecordingState, show_error_then_close, SPECTRUM_INTERVAL_MS};
use crate::audio;
use crate::events;
use crate::platform;
use crate::platform::hotkey;
use crate::state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Return type for spawn_audio_thread: (cmd_tx, spectrum_data, reply_rx, stream_error).
type AudioThreadHandles = (
    crossbeam_channel::Sender<AudioCmd>,
    Arc<std::sync::Mutex<Vec<f32>>>,
    crossbeam_channel::Receiver<AudioReply>,
    Arc<AtomicBool>,
);

/// Spawns the dedicated audio thread (cpal::Stream is not Send).
pub fn spawn_audio_thread() -> AudioThreadHandles {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<AudioCmd>();
    let (reply_tx, reply_rx) = crossbeam_channel::unbounded::<AudioReply>();
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
                Ok(AudioCmd::StartMicTest { device_uid }) => {
                    recorder.start_recording(device_uid.as_deref());
                    // No reply — fire-and-forget for mic test
                }
                Ok(AudioCmd::StopMicTest) => {
                    if let Some(path) = recorder.stop_recording() {
                        let _ = std::fs::remove_file(&path);
                    }
                }
                Err(_) => break,
            }
        }
    });

    (cmd_tx, spectrum_data, reply_rx, stream_error)
}

pub fn spawn_hotkey_handler(
    hotkey_rx: crossbeam_channel::Receiver<hotkey::HotkeyEvent>,
    app: AppHandle,
    state: Arc<AppState>,
    rec_state: Arc<std::sync::Mutex<RecordingState>>,
) {
    std::thread::spawn(move || loop {
        match hotkey_rx.recv() {
            Ok(hotkey::HotkeyEvent::KeyDown) => {
                let mode = state.settings.lock().unwrap().recording_mode;
                let is_recording = state.runtime.lock().unwrap().is_recording;
                let mut rec = rec_state.lock().unwrap();
                if mode == crate::state::RecordingMode::Toggle && is_recording {
                    super::lifecycle::stop_recording_and_enqueue(&app, &state, &mut rec);
                } else {
                    super::lifecycle::start_recording(&app, &state, &mut rec);
                }
            }
            Ok(hotkey::HotkeyEvent::KeyUp) => {
                let mode = state.settings.lock().unwrap().recording_mode;
                if mode != crate::state::RecordingMode::Toggle {
                    let mut rec = rec_state.lock().unwrap();
                    super::lifecycle::stop_recording_and_enqueue(&app, &state, &mut rec);
                }
            }
            Ok(hotkey::HotkeyEvent::CancelPressed) => {
                let rt = state.runtime.lock().unwrap();
                let is_recording = rt.is_recording;
                let is_transcribing = rt.is_transcribing;
                let has_queue = !rt.queue.is_empty();
                drop(rt);

                if is_recording {
                    log::info!("Cancel shortcut pressed during recording, discarding");
                    let mut rec = rec_state.lock().unwrap();
                    super::lifecycle::cancel_recording(&app, &state, &mut rec);
                } else if is_transcribing || has_queue {
                    log::info!("Cancel shortcut pressed, cancelling transcription");
                    super::lifecycle::cancel_transcription(&app, &state);
                }
            }
            Ok(hotkey::HotkeyEvent::CaptureUpdate { modifiers, key_codes }) => {
                let _ = app.emit(events::SHORTCUT_CAPTURE_UPDATE, serde_json::json!({
                    "modifiers": modifiers,
                    "key_codes": key_codes,
                }));
            }
            Ok(hotkey::HotkeyEvent::CaptureComplete(shortcut)) => {
                let _ = app.emit(events::SHORTCUT_CAPTURE_COMPLETE, serde_json::json!({
                    "key_codes": shortcut.key_codes,
                    "modifiers": shortcut.modifiers,
                    "kind": shortcut.kind,
                    "display": shortcut.display_string(),
                }));
            }
            Err(_) => break,
        }
    });
}

/// Spawns the spectrum emission timer (30fps) and monitors stream errors.
pub fn spawn_spectrum_emitter(
    app: AppHandle,
    state: Arc<AppState>,
    cmd_tx: crossbeam_channel::Sender<AudioCmd>,
    spectrum_data: Arc<std::sync::Mutex<Vec<f32>>>,
    stream_error: Arc<AtomicBool>,
) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(SPECTRUM_INTERVAL_MS));

        // Fast lock-free check — avoids mutex contention when idle
        if !state.audio_flags.is_active() {
            continue;
        }

        let is_mic_testing = state.audio_flags.is_mic_testing();

        // Detect audio stream error (e.g. device disconnected)
        if stream_error.load(Ordering::Relaxed) {
            log::warn!("Audio stream error detected (device disconnected?), forcing stop");
            state.runtime.lock().unwrap().is_recording = false;
            state.audio_flags.set_recording(false);
            stream_error.store(false, Ordering::Relaxed);

            // Actually stop the cpal stream — without this the mic stays active
            let _ = cmd_tx.send(AudioCmd::StopRecording);

            platform::play_sound("Basso");
            let _ = app.emit(events::RECORDING_STOPPED, ());
            show_error_then_close(&app);
            continue;
        }

        let spectrum = spectrum_data.lock().unwrap().clone();
        let _ = cmd_tx.send(AudioCmd::GetSpectrum);
        if is_mic_testing {
            let _ = app.emit(events::MIC_TEST_SPECTRUM, &spectrum);
        } else {
            // Feed spectrum directly to native pill (no Tauri event needed)
            crate::ui::pill::set_spectrum(&spectrum);
        }
    });
}
