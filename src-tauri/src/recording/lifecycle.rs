use super::{
    AudioCmd, AudioReply, RecordingState, PILL_CLOSE_GENERATION,
    SHORT_TAP_MS, DOUBLE_TAP_MS, show_error_then_close,
};
use crate::events;
use crate::platform;
use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub fn start_recording(app: &AppHandle, state: &Arc<AppState>, rec: &mut RecordingState) {
    {
        let mut rt = state.runtime.lock().unwrap();
        if rt.is_recording {
            return;
        }
        // Cancel mic test if running
        if rt.mic_testing {
            rt.mic_testing = false;
            state.audio_flags.set_mic_testing(false);
            let _ = rec.audio_tx.send(AudioCmd::StopMicTest);
            let _ = app.emit(events::MIC_TEST_STOPPED, ());
        }
        rt.is_recording = true;
        state.audio_flags.set_recording(true);
        rt.transcription_cancelled = false;
    }
    rec.key_down_time = Some(std::time::Instant::now());
    PILL_CLOSE_GENERATION.fetch_add(1, Ordering::Relaxed);

    // Show pill immediately in Preparing mode (before stream starts)
    crate::ui::pill::open(app, crate::ui::pill::PillMode::Preparing);
    crate::ui::tray::set_tray_state(app, "recording");

    let (device_uid, ducking_enabled, ducking_level) = {
        let s = state.settings.lock().unwrap();
        (s.selected_input_device_uid.clone(), s.audio_ducking_enabled, s.audio_ducking_level)
    };
    let _ = rec.audio_tx.send(AudioCmd::StartRecording { device_uid });
    let _ = rec.audio_rx.recv();
    log::debug!("Audio stream created, waiting for first samples");

    // Duck AFTER stream started: BT profile switch (A2DP→HFP) has already happened,
    // so we read/set volume in the actual audio state.
    if ducking_enabled {
        platform::audio_ducking::duck_volume(ducking_level);
    }

    // Wait for first audio samples in a background thread so the hotkey handler
    // returns immediately and can process cancel/stop events during the wait.
    let samples_flag = rec.samples_received.clone();
    let state_clone = Arc::clone(state);
    let app_clone = app.clone();
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        loop {
            if samples_flag.load(Ordering::Relaxed) {
                log::debug!("First audio samples received, transitioning to Recording");
                break;
            }
            // Early exit if recording was stopped/cancelled while waiting
            if !state_clone.audio_flags.is_recording() {
                log::debug!("Recording stopped before first samples, aborting transition");
                return;
            }
            if std::time::Instant::now() >= deadline {
                log::warn!("Timeout waiting for first audio samples, transitioning to Recording anyway");
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }

        // Double-check recording wasn't cancelled during the final sleep
        if !state_clone.audio_flags.is_recording() {
            log::debug!("Recording stopped before transition, aborting");
            return;
        }

        // Stream has data — transition to Recording mode + audible cue
        platform::play_sound("Tink");
        crate::ui::pill::set_mode(crate::ui::pill::PillMode::Recording);
        let _ = app_clone.emit(events::RECORDING_STARTED, ());
    });
}

pub fn stop_recording_and_enqueue(
    app: &AppHandle,
    state: &Arc<AppState>,
    rec: &mut RecordingState,
) {
    {
        let mut rt = state.runtime.lock().unwrap();
        if !rt.is_recording {
            return;
        }
        rt.is_recording = false;
        state.audio_flags.set_recording(false);
    }

    // Restore BEFORE stopping stream: on BT, the mic stream keeps HFP active,
    // so we restore volume in the same audio profile state as when we ducked.
    // Stopping the stream triggers HFP→A2DP switch which would swallow the restore.
    platform::audio_ducking::restore_volume();
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
            crate::ui::tray::close_pill_window(app);
            let _ = app.emit(events::RECORDING_STOPPED, ());
            return;
        }
    };

    platform::play_sound("Pop");

    let count = state.enqueue(audio_path);
    let _ = app.emit(
        events::RECORDING_STOPPED,
        serde_json::json!({ "queue_count": count }),
    );
    // count = queue len after enqueue; +1 if already transcribing = total pending
    let is_transcribing = state.runtime.lock().unwrap().is_transcribing;
    crate::ui::pill::set_pending(count as u32 + if is_transcribing { 1 } else { 0 });
    crate::ui::pill::set_mode(crate::ui::pill::PillMode::Transcribing);
    crate::ui::tray::set_tray_state(app, "transcribing");

    let app_clone = app.clone();
    let state_clone = Arc::clone(state);
    tauri::async_runtime::spawn(async move {
        super::pipeline::process_next_in_queue(&app_clone, &state_clone).await;
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
    rec.last_short_tap_time = Some(std::time::Instant::now());

    let rt = state.runtime.lock().unwrap();
    let is_transcribing = rt.is_transcribing;
    let queue_empty = rt.queue.is_empty();
    drop(rt);

    if !is_transcribing && queue_empty {
        crate::ui::tray::close_pill_window(app);
    } else {
        crate::ui::pill::set_mode(crate::ui::pill::PillMode::Transcribing);
        crate::ui::tray::set_tray_state(app, "transcribing");
    }
    let _ = app.emit(events::RECORDING_STOPPED, ());
}

pub(super) fn cancel_recording(app: &AppHandle, state: &Arc<AppState>, rec: &mut RecordingState) {
    {
        let mut rt = state.runtime.lock().unwrap();
        if !rt.is_recording {
            return;
        }
        rt.is_recording = false;
        state.audio_flags.set_recording(false);
    }

    // Restore BEFORE stopping stream (same rationale as stop_recording_and_enqueue)
    platform::audio_ducking::restore_volume();
    let _ = rec.audio_tx.send(AudioCmd::StopRecording);
    if let Ok(AudioReply::Stopped { path: Some(path) }) = rec.audio_rx.recv() {
        let _ = std::fs::remove_file(&path);
    }
    rec.key_down_time = None;
    rec.last_short_tap_time = None;

    platform::play_sound("Funk");
    let _ = app.emit(events::RECORDING_STOPPED, ());
    show_error_then_close(app);
}

pub(super) fn cancel_transcription(app: &AppHandle, state: &Arc<AppState>) {
    while let Some(path) = state.dequeue() {
        let _ = std::fs::remove_file(&path);
    }
    {
        let mut rt = state.runtime.lock().unwrap();
        rt.transcription_cancelled = true;
        rt.last_paste_had_content = false;
    }
    crate::ui::pill::set_pending(0);
    platform::play_sound("Funk");
    let _ = app.emit(events::TRANSCRIPTION_CANCELLED, ());
    show_error_then_close(app);
}
