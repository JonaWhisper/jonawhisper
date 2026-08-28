use super::{PermissionReport, PermissionStatus};
use core_foundation::base::TCFType;
use core_foundation::url::{CFURL, CFURLRef};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{LazyLock, Mutex};

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
    // SAFETY: AVCaptureDevice is an ObjC class with +authorizationStatusForMediaType: class method.
    // Returns AVAuthorizationStatus (NSInteger). AVFoundation framework linked via build.rs.
    let status: isize =
        unsafe { msg_send![cls, authorizationStatusForMediaType: &*media_type] };

    // AVAuthorizationStatus: 0=NotDetermined, 1=Restricted, 2=Denied, 3=Authorized
    match status {
        3 => PermissionStatus::Granted,
        2 | 1 => PermissionStatus::Denied,
        _ => PermissionStatus::Undetermined,
    }
}

/// Check accessibility via AXIsProcessTrusted (ApplicationServices framework).
fn check_accessibility_permission() -> PermissionStatus {
    // SAFETY: AXIsProcessTrusted is a C function from ApplicationServices framework.
    // Returns Boolean (true if process has accessibility permission).
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
    extern "C" fn noop_callback(
        _proxy: *mut c_void,
        _event_type: u32,
        event: *mut c_void,
        _user_info: *mut c_void,
    ) -> *mut c_void {
        event
    }

    // SAFETY: CGEventTapCreate is a CoreGraphics C function. We create a listen-only tap
    // (options=1) that returns immediately. If tap creation fails (null), we lack permission.
    // The returned CFMachPortRef is released immediately via CFRelease.
    unsafe {
        let tap = super::ffi::CGEventTapCreate(
            1,           // kCGSessionEventTap
            0,           // kCGHeadInsertEventTap
            1,           // kCGEventTapOptionListenOnly
            (1u64 << 10) | (1u64 << 11) | (1u64 << 12),  // keyDown + keyUp + flagsChanged
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

pub fn request_permission(kind: &str) -> bool {
    match kind {
        "microphone" => {
            request_microphone_access();
            true
        }
        "accessibility" => {
            request_accessibility_access();
            true
        }
        "input_monitoring" => {
            let _ = std::process::Command::new("open")
                .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"])
                .output();
            true
        }
        _ => false,
    }
}

/// Trigger the microphone permission dialog via AVCaptureDevice requestAccessForMediaType:.
fn request_microphone_access() {
    use block2::StackBlock;
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, Bool};
    use objc2_foundation::NSString;

    let cls = match AnyClass::get(c"AVCaptureDevice") {
        Some(c) => c,
        None => {
            log::warn!("AVCaptureDevice class not found");
            return;
        }
    };

    let media_type = NSString::from_str("soun");
    let block = StackBlock::new(|granted: Bool| {
        log::info!("Microphone access response: {}", granted.as_bool());
    });

    // SAFETY: ObjC message send to AVCaptureDevice class method.
    // The StackBlock is valid for the duration of this call (sync completion on macOS).
    unsafe {
        let _: () = msg_send![cls, requestAccessForMediaType: &*media_type, completionHandler: &block];
    }
}

/// Trigger the accessibility permission prompt and open System Settings.
fn request_accessibility_access() {
    // SAFETY: All extern functions are from Apple's ApplicationServices/CoreFoundation frameworks.
    // CFStringCreateWithCString creates a CFString from a C string.
    // CFDictionaryCreate creates a dictionary with the "AXTrustedCheckOptionPrompt" key set to true.
    // AXIsProcessTrustedWithOptions prompts the user for accessibility access.
    // All CF objects are released after use.
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
            c"AXTrustedCheckOptionPrompt".as_ptr() as *const u8,
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

        let trusted = AXIsProcessTrustedWithOptions(dict);
        CFRelease(dict);
        CFRelease(key);

        if !trusted {
            open_privacy_settings("Privacy_Accessibility");
        }
    }
}

fn open_privacy_settings(anchor: &str) {
    let url = format!(
        "x-apple.systempreferences:com.apple.preference.security?{}",
        anchor
    );
    if let Ok(mut child) = std::process::Command::new("open").arg(url).spawn() {
        std::thread::spawn(move || { let _ = child.wait(); });
    }
}

// -- System sounds (AudioToolbox) --
//
// The cues fire from the recording, spectrum and transcription threads, which have no
// autorelease pool and never drain one — an ObjC path (NSSound) would strand every
// autoreleased temporary for the life of the thread. AudioServices allocates none.

#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    fn AudioServicesCreateSystemSoundID(url: CFURLRef, out_id: *mut u32) -> i32;
    fn AudioServicesPlaySystemSoundWithCompletion(id: u32, completion: *const c_void);
    fn AudioServicesSetProperty(
        property_id: u32,
        specifier_size: u32,
        specifier: *const c_void,
        data_size: u32,
        data: *const c_void,
    ) -> i32;
}

/// `kAudioServicesPropertyIsUISound`.
const K_IS_UI_SOUND: u32 = u32::from_be_bytes(*b"isui");

static SYSTEM_SOUNDS: LazyLock<Mutex<HashMap<String, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn register_system_sound(name: &str) -> Option<u32> {
    let url = CFURL::from_path(format!("/System/Library/Sounds/{}.aiff", name), false)?;

    let mut id: u32 = 0;
    let status = unsafe { AudioServicesCreateSystemSoundID(url.as_concrete_TypeRef(), &mut id) };
    if status != 0 {
        log::warn!("Sound '{}' unavailable (OSStatus {})", name, status);
        return None;
    }

    // Defaults to 1, which would mute the cues whenever "Play user interface sound
    // effects" is off in System Settings. afplay never honoured that checkbox and the
    // app exposes no sound toggle of its own, so opting out keeps the shipped behaviour.
    let off: u32 = 0;
    unsafe {
        AudioServicesSetProperty(
            K_IS_UI_SOUND,
            std::mem::size_of::<u32>() as u32,
            &id as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
            &off as *const u32 as *const c_void,
        );
    }

    Some(id)
}

pub fn play_sound(name: &str) {
    // IDs are cached rather than disposed after playback: playback is asynchronous, so
    // disposing on return would cut the cue short, and re-registering per call leaks a
    // server-side resource — the very failure mode this replaced.
    let Ok(mut cache) = SYSTEM_SOUNDS.lock() else { return };

    let id = match cache.get(name) {
        Some(id) => *id,
        None => {
            let Some(id) = register_system_sound(name) else { return };
            cache.insert(name.to_string(), id);
            id
        }
    };
    drop(cache);

    unsafe { AudioServicesPlaySystemSoundWithCompletion(id, std::ptr::null()) };
}

// -- Launch at Login (SMAppService) --
//
// Uses the native macOS SMAppService API (macOS 13+) for proper BTM integration.
// Requires a Developer ID Application certificate (+ notarisation) to function.
// With Apple Development cert, register() no-ops and status stays notRegistered.
//
// We detect this at runtime by inspecting the code signature of the running binary
// via `codesign -dv --verbose=2`. If the Authority chain contains "Developer ID
// Application", the feature is available. Result is cached in a OnceLock.
//
// Status values returned to the frontend:
//   "unavailable"       → not signed with Developer ID (dev build), option hidden
//   "enabled"           → registered and active
//   "requires_approval" → registered, user must approve in System Settings > Login Items
//   "disabled"          → not registered

static IS_DEVELOPER_ID: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

fn is_developer_id_signed() -> bool {
    *IS_DEVELOPER_ID.get_or_init(|| {
        let Ok(exe) = std::env::current_exe() else { return false };
        let Ok(output) = std::process::Command::new("codesign")
            .args(["-dv", "--verbose=2", exe.to_str().unwrap_or("")])
            .output()
        else {
            return false;
        };
        // codesign writes signing info to stderr
        let stderr = String::from_utf8_lossy(&output.stderr);
        let result = stderr.contains("Developer ID Application");
        log::info!("Launch at login available (Developer ID signed): {}", result);
        result
    })
}

pub fn get_launch_at_login_status() -> &'static str {
    if !is_developer_id_signed() {
        return "unavailable";
    }
    use smappservice_rs::{AppService, ServiceStatus, ServiceType};
    let svc = AppService::new(ServiceType::MainApp);
    match svc.status() {
        ServiceStatus::Enabled => "enabled",
        ServiceStatus::RequiresApproval => "requires_approval",
        _ => "disabled",
    }
}

pub fn set_launch_at_login(enabled: bool) -> Result<&'static str, String> {
    if !is_developer_id_signed() {
        return Err("Launch at login requires a Developer ID certificate".to_string());
    }
    use smappservice_rs::{AppService, ServiceType};
    let svc = AppService::new(ServiceType::MainApp);
    if enabled {
        svc.register().map_err(|e| e.to_string())?;
        log::info!("SMAppService: registered main app as login item");
    } else {
        svc.unregister().map_err(|e| e.to_string())?;
        log::info!("SMAppService: unregistered main app login item");
    }
    Ok(get_launch_at_login_status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_a_system_sound_off_the_main_thread() {
        let id = std::thread::spawn(|| register_system_sound("Tink"))
            .join()
            .unwrap();

        assert!(matches!(id, Some(id) if id != 0));
    }

    #[test]
    fn unknown_sound_registers_nothing() {
        assert_eq!(register_system_sound("NoSuchSystemSound"), None);
    }

    #[test]
    fn repeated_playback_reuses_the_cached_id() {
        play_sound("Pop");
        let first = SYSTEM_SOUNDS.lock().unwrap()["Pop"];

        play_sound("Pop");
        assert_eq!(SYSTEM_SOUNDS.lock().unwrap()["Pop"], first);
    }
}
