use crate::state::AppState;
use std::sync::Arc;

#[tauri::command]
pub fn get_audio_devices() -> Vec<crate::platform::audio_devices::AudioDevice> {
    crate::audio::list_usable_devices()
}

// Building a cpal stream can take a while — a Bluetooth profile switch runs into
// hundreds of ms — so these stay off the IPC thread, as they were when a channel
// carried them to the audio thread.
#[tauri::command]
pub fn start_mic_test(state: tauri::State<'_, Arc<AppState>>, recorder: tauri::State<'_, crate::recording::SharedRecorder>) {
    let device_uid = state.settings.lock().unwrap().selected_input_device_uid.clone();
    state.runtime.lock().unwrap().mic_testing = true;
    state.audio_flags.set_mic_testing(true);

    let recorder = crate::recording::SharedRecorder::clone(&recorder);
    std::thread::spawn(move || {
        recorder.lock().unwrap().start_recording(device_uid.as_deref());
    });
}

#[tauri::command]
pub fn stop_mic_test(state: tauri::State<'_, Arc<AppState>>, recorder: tauri::State<'_, crate::recording::SharedRecorder>) {
    state.runtime.lock().unwrap().mic_testing = false;
    state.audio_flags.set_mic_testing(false);

    let recorder = crate::recording::SharedRecorder::clone(&recorder);
    std::thread::spawn(move || {
        if let Some(path) = recorder.lock().unwrap().stop_recording() {
            let _ = std::fs::remove_file(&path);
        }
    });
}
