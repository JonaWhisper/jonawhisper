use std::ffi::c_void;
use std::sync::Mutex;

/// Saved state for restoring after ducking: device_id + volume.
struct SavedState {
    device_id: u32,
    volume: f32,
}

static SAVED_STATE: Mutex<Option<SavedState>> = Mutex::new(None);

// CoreAudio FFI
#[allow(non_upper_case_globals, non_snake_case, dead_code)]
mod ca {
    use std::ffi::c_void;

    pub type AudioObjectID = u32;
    pub type OSStatus = i32;

    pub const kAudioObjectSystemObject: AudioObjectID = 1;

    pub const kAudioHardwarePropertyDefaultOutputDevice: u32 = u32::from_be_bytes(*b"dOut");
    /// Virtual main volume — the same control as the macOS menu bar slider.
    /// Preserves stereo balance when adjusting volume. Works on all device types
    /// (built-in speakers, Bluetooth, USB DACs).
    pub const kAudioHardwareServiceDeviceProperty_VirtualMainVolume: u32 =
        u32::from_be_bytes(*b"vmvc");

    pub const kAudioObjectPropertyScopeGlobal: u32 = u32::from_be_bytes(*b"glob");
    pub const kAudioDevicePropertyScopeOutput: u32 = u32::from_be_bytes(*b"outp");
    pub const kAudioObjectPropertyElementMain: u32 = 0;

    #[repr(C)]
    pub struct AudioObjectPropertyAddress {
        pub mSelector: u32,
        pub mScope: u32,
        pub mElement: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        pub fn AudioObjectGetPropertyData(
            inObjectID: AudioObjectID,
            inAddress: *const AudioObjectPropertyAddress,
            inQualifierDataSize: u32,
            inQualifierData: *const c_void,
            ioDataSize: *mut u32,
            outData: *mut c_void,
        ) -> OSStatus;

        pub fn AudioObjectSetPropertyData(
            inObjectID: AudioObjectID,
            inAddress: *const AudioObjectPropertyAddress,
            inQualifierDataSize: u32,
            inQualifierData: *const c_void,
            inDataSize: u32,
            inData: *const c_void,
        ) -> OSStatus;
    }
}

fn get_default_output_device() -> Option<u32> {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioHardwarePropertyDefaultOutputDevice,
            mScope: ca::kAudioObjectPropertyScopeGlobal,
            mElement: ca::kAudioObjectPropertyElementMain,
        };
        let mut device_id: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = ca::AudioObjectGetPropertyData(
            ca::kAudioObjectSystemObject,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut device_id as *mut u32 as *mut c_void,
        );
        if status != 0 || device_id == 0 {
            return None;
        }
        Some(device_id)
    }
}

fn get_virtual_volume(device_id: u32) -> Option<f32> {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioHardwareServiceDeviceProperty_VirtualMainVolume,
            mScope: ca::kAudioDevicePropertyScopeOutput,
            mElement: ca::kAudioObjectPropertyElementMain,
        };
        let mut volume: f32 = 0.0;
        let mut size = std::mem::size_of::<f32>() as u32;
        let status = ca::AudioObjectGetPropertyData(
            device_id,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut volume as *mut f32 as *mut c_void,
        );
        if status != 0 {
            return None;
        }
        Some(volume)
    }
}

fn set_virtual_volume(device_id: u32, volume: f32) -> bool {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioHardwareServiceDeviceProperty_VirtualMainVolume,
            mScope: ca::kAudioDevicePropertyScopeOutput,
            mElement: ca::kAudioObjectPropertyElementMain,
        };
        let status = ca::AudioObjectSetPropertyData(
            device_id,
            &address,
            0,
            std::ptr::null(),
            std::mem::size_of::<f32>() as u32,
            &volume as *const f32 as *const c_void,
        );
        status == 0
    }
}

/// Lower the default output volume by `reduction` (0.0 = no change, 1.0 = mute).
/// Saves the original volume so it can be restored later.
///
/// Uses `VirtualMainVolume` — the same API as the macOS system volume slider.
/// This preserves stereo balance on all device types (built-in, Bluetooth, USB).
pub fn duck_volume(reduction: f32) {
    let device_id = match get_default_output_device() {
        Some(id) => id,
        None => {
            log::warn!("audio_ducking: no default output device");
            return;
        }
    };

    let current = match get_virtual_volume(device_id) {
        Some(v) => v,
        None => {
            log::warn!("audio_ducking: could not read VirtualMainVolume");
            return;
        }
    };

    *SAVED_STATE.lock().unwrap() = Some(SavedState { device_id, volume: current });

    let ducked = (current * (1.0 - reduction)).clamp(0.0, 1.0);
    if set_virtual_volume(device_id, ducked) {
        log::info!("audio_ducking: {:.2} -> {:.2} (reduction={}, device={})", current, ducked, reduction, device_id);
    } else {
        log::warn!("audio_ducking: failed to set VirtualMainVolume");
    }
}

/// Restore the volume saved by the last `duck_volume()` call.
/// Skips restore if the default output device changed since ducking (e.g. user switched audio output).
pub fn restore_volume() {
    let saved = match SAVED_STATE.lock().unwrap().take() {
        Some(s) => s,
        None => return,
    };

    let current_device = match get_default_output_device() {
        Some(id) => id,
        None => {
            log::warn!("audio_ducking: no default output device for restore");
            return;
        }
    };

    if current_device != saved.device_id {
        log::warn!(
            "audio_ducking: output device changed ({} -> {}), skipping restore",
            saved.device_id,
            current_device
        );
        return;
    }

    if set_virtual_volume(current_device, saved.volume) {
        log::info!("audio_ducking: restored to {:.2} (device={})", saved.volume, current_device);
    } else {
        log::warn!("audio_ducking: failed to restore VirtualMainVolume");
    }
}
