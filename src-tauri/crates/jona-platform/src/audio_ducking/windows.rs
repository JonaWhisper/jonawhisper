//! Ducking through the default render endpoint's master volume — the control
//! the volume keys drive, which is the closest thing Windows has to the
//! VirtualMainVolume the macOS side moves.

use std::sync::Mutex;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eMultimedia, eRender};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};

/// Saved state for restoring after ducking. Keeps both volumes so restore can
/// tell our own change from one the user made meanwhile.
struct SavedState {
    device_id: String,
    original_volume: f32,
    ducked_volume: f32,
}

/// Tolerance when comparing volumes: the scalar round-trips through the
/// endpoint's own step curve, so what we read back is not always bit-identical
/// to what we set.
const VOLUME_TOLERANCE: f32 = 0.02;

static SAVED_STATE: Mutex<Option<SavedState>> = Mutex::new(None);

/// Identity and volume control of the endpoint the system is playing through.
fn default_endpoint() -> Option<(String, IAudioEndpointVolume)> {
    unsafe {
        // Ducking runs from the recording threads, which never initialise COM.
        // A thread already living in another apartment keeps it — the call
        // returns RPC_E_CHANGED_MODE and the objects below still work.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;

        let id = device.GetId().ok()?;
        let name = id.to_string().ok();
        CoTaskMemFree(Some(id.0.cast()));

        let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).ok()?;
        Some((name?, volume))
    }
}

pub fn duck_volume(reduction: f32) {
    let Some((device_id, volume)) = default_endpoint() else {
        log::warn!("audio_ducking: no default output endpoint");
        return;
    };
    let Ok(current) = (unsafe { volume.GetMasterVolumeLevelScalar() }) else {
        log::warn!("audio_ducking: could not read the master volume");
        return;
    };

    let ducked = (current * (1.0 - reduction)).clamp(0.0, 1.0);
    *SAVED_STATE.lock().unwrap() = Some(SavedState {
        device_id: device_id.clone(),
        original_volume: current,
        ducked_volume: ducked,
    });

    match unsafe { volume.SetMasterVolumeLevelScalar(ducked, std::ptr::null()) } {
        Ok(()) => log::info!(
            "audio_ducking: {current:.2} -> {ducked:.2} (reduction={reduction}, device={device_id})"
        ),
        Err(e) => log::warn!("audio_ducking: failed to set the master volume: {e}"),
    }
}

pub fn restore_volume() {
    let Some(saved) = SAVED_STATE.lock().unwrap().take() else { return };
    let Some((device_id, volume)) = default_endpoint() else {
        log::warn!("audio_ducking: no default output endpoint for restore");
        return;
    };

    if device_id != saved.device_id {
        log::warn!(
            "audio_ducking: output endpoint changed ({} -> {device_id}), skipping restore",
            saved.device_id
        );
        return;
    }

    let Ok(current) = (unsafe { volume.GetMasterVolumeLevelScalar() }) else {
        log::warn!("audio_ducking: could not read the volume for restore verification");
        return;
    };
    if (current - saved.ducked_volume).abs() > VOLUME_TOLERANCE {
        log::info!(
            "audio_ducking: volume changed externally ({current:.2} != ducked {:.2}), skipping restore",
            saved.ducked_volume
        );
        return;
    }

    match unsafe { volume.SetMasterVolumeLevelScalar(saved.original_volume, std::ptr::null()) } {
        Ok(()) => log::info!(
            "audio_ducking: restored to {:.2} (device={device_id})",
            saved.original_volume
        ),
        Err(e) => log::warn!("audio_ducking: failed to restore the master volume: {e}"),
    }
}
