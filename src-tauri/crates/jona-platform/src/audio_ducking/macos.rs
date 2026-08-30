use std::ffi::c_void;
use std::sync::Mutex;

/// Saved state for restoring after ducking.
/// Stores both the original and ducked volume so we can verify on restore
/// that the volume hasn't been changed by a BT profile switch or the user.
struct SavedState {
    device_id: u32,
    original_volume: f32,
    ducked_volume: f32,
}

/// Tolerance for comparing volume values. BT profile switches or codec
/// quantization (AVRCP for A2DP vs AT+VGS for HFP) can cause small
/// differences between what we set and what we read back.
const VOLUME_TOLERANCE: f32 = 0.02;

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
    /// Transport type — identifies how the device is connected.
    pub const kAudioDevicePropertyTransportType: u32 = u32::from_be_bytes(*b"tran");
    pub const kAudioDeviceTransportTypeBluetooth: u32 = u32::from_be_bytes(*b"blue");
    pub const kAudioDeviceTransportTypeBluetoothLE: u32 = u32::from_be_bytes(*b"blea");

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

fn is_bluetooth_device(device_id: u32) -> bool {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioDevicePropertyTransportType,
            mScope: ca::kAudioObjectPropertyScopeGlobal,
            mElement: ca::kAudioObjectPropertyElementMain,
        };
        let mut transport: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = ca::AudioObjectGetPropertyData(
            device_id,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            &mut transport as *mut u32 as *mut c_void,
        );
        if status != 0 {
            return false;
        }
        transport == ca::kAudioDeviceTransportTypeBluetooth
            || transport == ca::kAudioDeviceTransportTypeBluetoothLE
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

    let is_bt = is_bluetooth_device(device_id);

    let current = match get_virtual_volume(device_id) {
        Some(v) => v,
        None => {
            log::warn!("audio_ducking: could not read VirtualMainVolume");
            return;
        }
    };

    let ducked = (current * (1.0 - reduction)).clamp(0.0, 1.0);
    *SAVED_STATE.lock().unwrap() = Some(SavedState {
        device_id,
        original_volume: current,
        ducked_volume: ducked,
    });

    if set_virtual_volume(device_id, ducked) {
        log::info!(
            "audio_ducking: {:.2} -> {:.2} (reduction={}, device={}, bt={})",
            current, ducked, reduction, device_id, is_bt
        );
    } else {
        log::warn!("audio_ducking: failed to set VirtualMainVolume");
    }
}

/// Restore the volume saved by the last `duck_volume()` call.
///
/// Safety checks before restoring:
/// 1. Device must still be the same (user didn't switch output).
/// 2. Current volume must be close to the ducked value we set. If it differs,
///    a BT profile switch (A2DP↔HFP have different volume mechanisms: AVRCP vs
///    AT+VGS) or the user changed the volume manually — don't overwrite.
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

    let current_volume = match get_virtual_volume(current_device) {
        Some(v) => v,
        None => {
            log::warn!("audio_ducking: could not read volume for restore verification");
            return;
        }
    };

    // Verify the volume is still at the ducked level we set.
    // On BT devices, profile switches (A2DP↔HFP) use different volume mechanisms
    // (AVRCP vs AT+VGS) and may change the value behind our back.
    if (current_volume - saved.ducked_volume).abs() > VOLUME_TOLERANCE {
        log::info!(
            "audio_ducking: volume changed externally ({:.2} != ducked {:.2}), skipping restore",
            current_volume,
            saved.ducked_volume
        );
        return;
    }

    if set_virtual_volume(current_device, saved.original_volume) {
        log::info!("audio_ducking: restored to {:.2} (device={})", saved.original_volume, current_device);
    } else {
        log::warn!("audio_ducking: failed to restore VirtualMainVolume");
    }
}
