use serde::{Deserialize, Serialize};
use std::ffi::c_void;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionReport {
    pub microphone: PermissionStatus,
    pub accessibility: PermissionStatus,
    pub input_monitoring: PermissionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PermissionStatus {
    Granted,
    Denied,
    Undetermined,
}

pub fn check_permissions() -> PermissionReport {
    PermissionReport {
        microphone: check_microphone_permission(),
        accessibility: check_accessibility_permission(),
        input_monitoring: check_input_monitoring_permission(),
    }
}

/// Check microphone authorization via [AVCaptureDevice authorizationStatusForMediaType:].
/// Uses objc2 msg_send! for proper Objective-C FFI. AVFoundation is linked via build.rs.
fn check_microphone_permission() -> PermissionStatus {
    use objc2::msg_send;
    use objc2::runtime::AnyClass;
    use objc2_foundation::NSString;

    let cls = match AnyClass::get(c"AVCaptureDevice") {
        Some(c) => c,
        None => {
            log::warn!("AVCaptureDevice class not found — AVFoundation not loaded?");
            return PermissionStatus::Undetermined;
        }
    };

    // AVMediaTypeAudio = @"soun"
    let media_type = NSString::from_str("soun");
    let status: isize =
        unsafe { msg_send![cls, authorizationStatusForMediaType: &*media_type] };

    // AVAuthorizationStatus: 0=NotDetermined, 1=Restricted, 2=Denied, 3=Authorized
    match status {
        3 => PermissionStatus::Granted,
        2 | 1 => PermissionStatus::Denied,
        _ => PermissionStatus::Undetermined,
    }
}

/// Check accessibility via AXIsProcessTrusted.
fn check_accessibility_permission() -> PermissionStatus {
    unsafe {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }

        if AXIsProcessTrusted() {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }
}

/// Check input monitoring by attempting to create a listen-only CGEvent tap.
/// We use listen-only (not active) to avoid interfering with event delivery to other apps.
/// The actual hotkey monitor uses an active tap, but the permission requirement is the same.
fn check_input_monitoring_permission() -> PermissionStatus {
    unsafe {
        extern "C" fn noop_callback(
            _proxy: *mut c_void,
            _event_type: u32,
            event: *mut c_void,
            _user_info: *mut c_void,
        ) -> *mut c_void {
            event
        }

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventTapCreate(
                tap: u32,
                place: u32,
                options: u32,
                events_of_interest: u64,
                callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
                user_info: *mut c_void,
            ) -> *mut c_void;
        }

        let tap = CGEventTapCreate(
            1,           // kCGSessionEventTap
            0,           // kCGHeadInsertEventTap
            1,           // kCGEventTapOptionListenOnly (safe — doesn't block other apps)
            1u64 << 12,  // CGEventMaskBit(kCGEventFlagsChanged)
            noop_callback,
            std::ptr::null_mut(),
        );

        if tap.is_null() {
            PermissionStatus::Denied
        } else {
            core_foundation::base::CFRelease(tap as *const _);
            PermissionStatus::Granted
        }
    }
}

/// Bundle ID must match tauri.conf.json "identifier"
const BUNDLE_ID: &str = "com.local.whisper-dictate";

pub fn request_permission(kind: &str) -> bool {
    match kind {
        "microphone" => {
            // Reset stale TCC entry (ad-hoc signature changes between builds)
            reset_tcc("Microphone");
            request_microphone_access();
            true
        }
        "accessibility" => {
            reset_tcc("Accessibility");
            request_accessibility_access();
            true
        }
        "input_monitoring" => {
            reset_tcc("ListenEvent");
            let _ = std::process::Command::new("open")
                .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"])
                .output();
            true
        }
        _ => false,
    }
}

/// Clear stale TCC entries for our bundle ID so macOS re-evaluates the current code signature.
fn reset_tcc(service: &str) {
    let output = std::process::Command::new("tccutil")
        .args(["reset", service, BUNDLE_ID])
        .output();
    match output {
        Ok(o) => {
            if !o.status.success() {
                log::debug!("tccutil reset {} {}: {:?}", service, BUNDLE_ID,
                    String::from_utf8_lossy(&o.stderr));
            }
        }
        Err(e) => log::debug!("tccutil failed: {}", e),
    }
}

/// Trigger the microphone permission dialog by briefly opening an audio input stream via cpal.
fn request_microphone_access() {
    std::thread::spawn(|| {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        if let Some(device) = host.default_input_device() {
            let config = cpal::StreamConfig {
                channels: 1,
                sample_rate: cpal::SampleRate(16000),
                buffer_size: cpal::BufferSize::Default,
            };
            match device.build_input_stream(
                &config,
                |_data: &[f32], _info: &cpal::InputCallbackInfo| {},
                |_err| {},
                None,
            ) {
                Ok(stream) => {
                    let _ = stream.play();
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    drop(stream);
                    log::info!("Microphone permission requested via cpal stream");
                }
                Err(e) => {
                    log::warn!("Could not open audio input for mic permission: {}", e);
                }
            }
        } else {
            log::warn!("No default input device found");
        }
    });
}

/// Trigger the accessibility permission dialog via AXIsProcessTrustedWithOptions with prompt=true.
fn request_accessibility_access() {
    unsafe {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
        }

        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFStringCreateWithCString(
                alloc: *const c_void,
                c_str: *const u8,
                encoding: u32,
            ) -> *const c_void;
            fn CFDictionaryCreate(
                allocator: *const c_void,
                keys: *const *const c_void,
                values: *const *const c_void,
                num_values: isize,
                key_callbacks: *const c_void,
                value_callbacks: *const c_void,
            ) -> *const c_void;
            fn CFRelease(cf: *const c_void);
            static kCFBooleanTrue: *const c_void;
            static kCFTypeDictionaryKeyCallBacks: u8;
            static kCFTypeDictionaryValueCallBacks: u8;
        }

        // kCFStringEncodingUTF8 = 0x08000100
        let key = CFStringCreateWithCString(
            std::ptr::null(),
            b"AXTrustedCheckOptionPrompt\0".as_ptr(),
            0x08000100,
        );

        let keys = [key];
        let values = [kCFBooleanTrue];

        let dict = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const u8 as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const u8 as *const c_void,
        );

        AXIsProcessTrustedWithOptions(dict);
        CFRelease(dict);
        CFRelease(key);
    }
}

pub fn play_sound(name: &str) {
    let _ = std::process::Command::new("/usr/bin/afplay")
        .arg(format!("/System/Library/Sounds/{}.aiff", name))
        .spawn();
}
