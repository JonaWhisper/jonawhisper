use crate::state::AppState;
use std::sync::Arc;

#[tauri::command]
pub fn get_audio_devices() -> Vec<crate::platform::audio_devices::AudioDevice> {
    crate::audio::list_usable_devices()
}

#[tauri::command]
pub fn start_mic_test(state: tauri::State<'_, Arc<AppState>>, sender: tauri::State<'_, crate::recording::MicTestSender>) {
    let device_uid = state.settings.lock().unwrap().selected_input_device_uid.clone();
    state.runtime.lock().unwrap().mic_testing = true;
    state.audio_flags.set_mic_testing(true);
    let _ = sender.0.send(crate::recording::AudioCmd::StartMicTest { device_uid });
}

#[tauri::command]
pub fn stop_mic_test(state: tauri::State<'_, Arc<AppState>>, sender: tauri::State<'_, crate::recording::MicTestSender>) {
    state.runtime.lock().unwrap().mic_testing = false;
    state.audio_flags.set_mic_testing(false);
    let _ = sender.0.send(crate::recording::AudioCmd::StopMicTest);
}
