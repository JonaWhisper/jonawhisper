use std::ffi::c_void;
use std::sync::Mutex;

/// Saved volume state for restoring after ducking.
static SAVED_VOLUME: Mutex<Option<f32>> = Mutex::new(None);

// CoreAudio FFI — reuses the same pattern as audio_devices.rs
#[allow(non_upper_case_globals, non_snake_case, dead_code)]
mod ca {
    use std::ffi::c_void;

    pub type AudioObjectID = u32;
    pub type OSStatus = i32;

    pub const kAudioObjectSystemObject: AudioObjectID = 1;

    pub const kAudioHardwarePropertyDefaultOutputDevice: u32 = u32::from_be_bytes(*b"dOut");
    pub const kAudioDevicePropertyVolumeScalar: u32 = u32::from_be_bytes(*b"volm");

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

fn get_volume(device_id: u32) -> Option<f32> {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioDevicePropertyVolumeScalar,
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

fn set_volume(device_id: u32, volume: f32) -> bool {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioDevicePropertyVolumeScalar,
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
pub fn duck_volume(reduction: f32) {
    let device_id = match get_default_output_device() {
        Some(id) => id,
        None => {
            log::warn!("audio_ducking: no default output device");
            return;
        }
    };

    let current = match get_volume(device_id) {
        Some(v) => v,
        None => {
            log::warn!("audio_ducking: could not read volume");
            return;
        }
    };

    *SAVED_VOLUME.lock().unwrap() = Some(current);

    let ducked = (current * (1.0 - reduction)).clamp(0.0, 1.0);
    if set_volume(device_id, ducked) {
        log::info!("audio_ducking: {} -> {} (reduction={})", current, ducked, reduction);
    } else {
        log::warn!("audio_ducking: failed to set volume");
    }
}

/// Restore the volume saved by the last `duck_volume()` call.
pub fn restore_volume() {
    let saved = SAVED_VOLUME.lock().unwrap().take();
    let volume = match saved {
        Some(v) => v,
        None => return, // nothing to restore
    };

    let device_id = match get_default_output_device() {
        Some(id) => id,
        None => {
            log::warn!("audio_ducking: no default output device for restore");
            return;
        }
    };

    if set_volume(device_id, volume) {
        log::info!("audio_ducking: restored to {}", volume);
    } else {
        log::warn!("audio_ducking: failed to restore volume");
    }
}
