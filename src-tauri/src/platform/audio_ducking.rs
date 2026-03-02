use std::ffi::c_void;
use std::sync::Mutex;

/// Saved volume state for restoring after ducking.
/// Master (element 0) or per-channel (elements 1+2) depending on device capabilities.
static SAVED_VOLUME: Mutex<Option<SavedVolume>> = Mutex::new(None);

enum SavedVolume {
    Master(f32),
    PerChannel { left: f32, right: f32 },
}

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

        pub fn AudioObjectHasProperty(
            inObjectID: AudioObjectID,
            inAddress: *const AudioObjectPropertyAddress,
        ) -> u8; // Boolean
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

/// Check if a volume property exists for the given element on an output device.
fn has_volume(device_id: u32, element: u32) -> bool {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioDevicePropertyVolumeScalar,
            mScope: ca::kAudioDevicePropertyScopeOutput,
            mElement: element,
        };
        ca::AudioObjectHasProperty(device_id, &address) != 0
    }
}

fn get_volume(device_id: u32, element: u32) -> Option<f32> {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioDevicePropertyVolumeScalar,
            mScope: ca::kAudioDevicePropertyScopeOutput,
            mElement: element,
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

fn set_volume(device_id: u32, element: u32, volume: f32) -> bool {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioDevicePropertyVolumeScalar,
            mScope: ca::kAudioDevicePropertyScopeOutput,
            mElement: element,
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
/// Uses master volume (element 0) when available, otherwise adjusts left (1) and right (2)
/// channels individually to avoid stereo imbalance.
pub fn duck_volume(reduction: f32) {
    let device_id = match get_default_output_device() {
        Some(id) => id,
        None => {
            log::warn!("audio_ducking: no default output device");
            return;
        }
    };

    // Prefer master volume (element 0); fall back to per-channel (elements 1+2)
    if has_volume(device_id, 0) {
        let current = match get_volume(device_id, 0) {
            Some(v) => v,
            None => {
                log::warn!("audio_ducking: could not read master volume");
                return;
            }
        };
        *SAVED_VOLUME.lock().unwrap() = Some(SavedVolume::Master(current));
        let ducked = (current * (1.0 - reduction)).clamp(0.0, 1.0);
        if set_volume(device_id, 0, ducked) {
            log::info!("audio_ducking: master {} -> {} (reduction={})", current, ducked, reduction);
        } else {
            log::warn!("audio_ducking: failed to set master volume");
        }
    } else {
        // Per-channel ducking (left=1, right=2)
        let left = get_volume(device_id, 1).unwrap_or(1.0);
        let right = get_volume(device_id, 2).unwrap_or(1.0);
        *SAVED_VOLUME.lock().unwrap() = Some(SavedVolume::PerChannel { left, right });

        let ducked_l = (left * (1.0 - reduction)).clamp(0.0, 1.0);
        let ducked_r = (right * (1.0 - reduction)).clamp(0.0, 1.0);
        let ok_l = set_volume(device_id, 1, ducked_l);
        let ok_r = set_volume(device_id, 2, ducked_r);
        if ok_l && ok_r {
            log::info!(
                "audio_ducking: L {:.2}->{:.2}, R {:.2}->{:.2} (reduction={})",
                left, ducked_l, right, ducked_r, reduction
            );
        } else {
            log::warn!("audio_ducking: per-channel set failed (L={}, R={})", ok_l, ok_r);
        }
    }
}

/// Restore the volume saved by the last `duck_volume()` call.
pub fn restore_volume() {
    let saved = SAVED_VOLUME.lock().unwrap().take();
    let saved = match saved {
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

    match saved {
        SavedVolume::Master(volume) => {
            if set_volume(device_id, 0, volume) {
                log::info!("audio_ducking: restored master to {}", volume);
            } else {
                log::warn!("audio_ducking: failed to restore master volume");
            }
        }
        SavedVolume::PerChannel { left, right } => {
            let ok_l = set_volume(device_id, 1, left);
            let ok_r = set_volume(device_id, 2, right);
            if ok_l && ok_r {
                log::info!("audio_ducking: restored L={:.2}, R={:.2}", left, right);
            } else {
                log::warn!("audio_ducking: per-channel restore failed (L={}, R={})", ok_l, ok_r);
            }
        }
    }
}
