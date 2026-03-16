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

/// Named return type for `spawn_audio_thread`.
pub struct AudioThreadHandles {
    pub cmd_tx: crossbeam_channel::Sender<AudioCmd>,
    /// Live spectrum data — lock-free atomic array shared with the cpal callback.
    pub spectrum_data: Arc<audio::AtomicSpectrum>,
    pub reply_rx: crossbeam_channel::Receiver<AudioReply>,
    pub stream_error: Arc<AtomicBool>,
    pub samples_received: Arc<AtomicBool>,
}

/// Spawns the dedicated audio thread (cpal::Stream is not Send).
pub fn spawn_audio_thread() -> AudioThreadHandles {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<AudioCmd>();
    let (reply_tx, reply_rx) = crossbeam_channel::unbounded::<AudioReply>();

    let stream_error = Arc::new(AtomicBool::new(false));
    let stream_error_clone = Arc::clone(&stream_error);

    let samples_received = Arc::new(AtomicBool::new(false));
    let samples_received_clone = Arc::clone(&samples_received);

    // Channel to send back the recorder's live spectrum handle once created.
    let (spectrum_tx, spectrum_rx) = crossbeam_channel::bounded::<Arc<audio::AtomicSpectrum>>(1);

    std::thread::spawn(move || {
        let mut recorder = audio::AudioRecorder::new(stream_error_clone, samples_received_clone);
        // Send the recorder's spectrum handle back to the main thread.
        let _ = spectrum_tx.send(recorder.spectrum_handle());

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

    // Receive the live spectrum handle from the audio thread.
    let spectrum_data = spectrum_rx.recv().expect("audio thread failed to send spectrum handle");

    AudioThreadHandles { cmd_tx, spectrum_data, reply_rx, stream_error, samples_received }
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
    spectrum_data: Arc<audio::AtomicSpectrum>,
    stream_error: Arc<AtomicBool>,
    samples_received: Arc<AtomicBool>,
) {
    // Number of flat spectrum frames to skip after samples start arriving.
    // FFT needs 1024 samples (~64ms at 16kHz), spectrum emitter runs at ~33ms intervals,
    // so ~4-6 frames will be flat before the first real spectrum is computed.
    const FLAT_GRACE_FRAMES: u32 = 8;
    const SMOOTHING: f32 = 0.55; // new data weight (old = 1 - this)
    // Visual flat threshold: bars below this value render at minimum height (4px),
    // appearing flat to the user. Must match pill.rs: (val * max_h).max(2*DPR).
    const VISUAL_FLAT_THRESHOLD: f32 = 0.12;

    std::thread::spawn(move || {
        let mut flat_frames_since_active = 0u32;
        let mut was_active = false;
        let mut smoothed = [0.0f32; 12];
        let mut frames_since_active = 0u32;

        loop {
            std::thread::sleep(Duration::from_millis(SPECTRUM_INTERVAL_MS));

            // Fast lock-free check — avoids contention when idle
            if !state.audio_flags.is_active() {
                was_active = false;
                flat_frames_since_active = 0;
                frames_since_active = 0;
                smoothed = [0.0; 12];
                continue;
            }

            // Reset on new recording session
            if !was_active {
                was_active = true;
                flat_frames_since_active = 0;
                frames_since_active = 0;
                smoothed = [0.0; 12];
            }

            frames_since_active += 1;
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

            // Read spectrum atomically — lock-free, never blocks, never lost
            let raw = spectrum_data.load();

            // Smooth locally (was previously done in the cpal callback under a mutex)
            let old_weight = 1.0 - SMOOTHING;
            for (s, &r) in smoothed.iter_mut().zip(raw.iter()) {
                *s = *s * old_weight + r * SMOOTHING;
            }

            let max_raw = raw.iter().cloned().fold(0.0f32, f32::max);
            let max_smoothed = smoothed.iter().cloned().fold(0.0f32, f32::max);

            // Diagnostic: log spectrum values periodically (~1s intervals) while recording
            if !is_mic_testing && state.audio_flags.is_recording()
                && samples_received.load(Ordering::Relaxed)
                && frames_since_active > FLAT_GRACE_FRAMES
                && frames_since_active.is_multiple_of(30)
            {
                log::debug!(
                    "Spectrum diagnostic (frame {}): raw_max={:.4}, smoothed_max={:.4}, raw=[{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}]",
                    frames_since_active, max_raw, max_smoothed,
                    raw[0], raw[1], raw[2], raw[3], raw[4], raw[5],
                    raw[6], raw[7], raw[8], raw[9], raw[10], raw[11],
                );
            }

            // Flat detection: use visual threshold (bars appear flat below this)
            let is_visually_flat = max_smoothed < VISUAL_FLAT_THRESHOLD;
            let is_numerically_flat = max_smoothed < 0.001;
            if (is_numerically_flat || is_visually_flat) && !is_mic_testing
                && state.audio_flags.is_recording()
                && samples_received.load(Ordering::Relaxed)
            {
                flat_frames_since_active += 1;
                if flat_frames_since_active == FLAT_GRACE_FRAMES + 1 {
                    if is_numerically_flat {
                        log::warn!("Spectrum flat while recording (frame {}, raw_max={:.6})", flat_frames_since_active, max_raw);
                    } else {
                        log::warn!("Spectrum visually flat while recording (frame {}, smoothed_max={:.4}, raw_max={:.4})", flat_frames_since_active, max_smoothed, max_raw);
                    }
                } else if flat_frames_since_active > FLAT_GRACE_FRAMES && flat_frames_since_active.is_multiple_of(30) {
                    log::warn!("Spectrum still flat (frame {}, ~{:.1}s, smoothed_max={:.4}, raw_max={:.4})",
                        flat_frames_since_active,
                        flat_frames_since_active as f32 * SPECTRUM_INTERVAL_MS as f32 / 1000.0,
                        max_smoothed, max_raw);
                }
            } else if !is_visually_flat {
                if flat_frames_since_active > FLAT_GRACE_FRAMES {
                    log::info!("Spectrum recovered after {} flat frames (smoothed_max={:.4})", flat_frames_since_active, max_smoothed);
                }
                flat_frames_since_active = 0;
            }
            if is_mic_testing {
                let _ = app.emit(events::MIC_TEST_SPECTRUM, smoothed.as_slice());
            } else {
                // Feed spectrum directly to native pill (no Tauri event needed)
                crate::ui::pill::set_spectrum(&smoothed);
            }
        }
    });
}
