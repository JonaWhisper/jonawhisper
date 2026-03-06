use super::{PermissionReport, PermissionStatus};
use std::ffi::c_void;

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
    let _ = std::process::Command::new("open").arg(url).spawn();
}

pub fn play_sound(name: &str) {
    let _ = std::process::Command::new("/usr/bin/afplay")
        .arg(format!("/System/Library/Sounds/{}.aiff", name))
        .spawn();
}

// -- Launch at Login (LaunchAgent plist) --
//
// SMAppService.mainApp silently no-ops without a notarized Developer ID cert
// (confirmed on macOS 15+/Darwin 25). We use the standard LaunchAgent plist
// approach instead — used by Figma, JetBrains, Steam, etc.
//
// Plist at ~/Library/LaunchAgents/com.local.jona-whisper.plist with RunAtLoad=true.
// Points to the current binary path — for a stable login item, install to /Applications.
// Appears in System Settings > General > Login Items > "Allow in the Background".
//
// IMPORTANT: Must call `launchctl bootstrap`/`bootout` alongside plist write/delete.
// Without bootout, macOS BTM (Background Task Manager) recreates the plist after deletion.

const LAUNCH_AGENT_LABEL: &str = "com.local.jona-whisper";

fn launch_agent_plist_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(home)
        .join(format!("Library/LaunchAgents/{}.plist", LAUNCH_AGENT_LABEL))
}

fn current_uid() -> u32 {
    // SAFETY: getuid() is a POSIX function, always safe to call.
    unsafe {
        extern "C" { fn getuid() -> u32; }
        getuid()
    }
}

/// Returns "enabled" if the LaunchAgent plist exists, "disabled" otherwise.
pub fn get_launch_at_login_status() -> &'static str {
    if launch_agent_plist_path().exists() { "enabled" } else { "disabled" }
}

/// Write or remove the LaunchAgent plist to enable/disable launch at login.
/// Uses launchctl bootstrap/bootout so BTM properly tracks the agent.
pub fn set_launch_at_login(enabled: bool) -> Result<&'static str, String> {
    let plist_path = launch_agent_plist_path();
    let uid = current_uid();
    let session = format!("gui/{}", uid);

    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe_str = exe.to_string_lossy();

        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             \t<key>Label</key>\n\
             \t<string>{label}</string>\n\
             \t<key>Program</key>\n\
             \t<string>{exe}</string>\n\
             \t<key>RunAtLoad</key>\n\
             \t<true/>\n\
             \t<key>KeepAlive</key>\n\
             \t<false/>\n\
             </dict>\n\
             </plist>\n",
            label = LAUNCH_AGENT_LABEL,
            exe = exe_str
        );

        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&plist_path, &content).map_err(|e| e.to_string())?;

        // Register with launchd so BTM tracks it (idempotent if already loaded).
        let _ = std::process::Command::new("launchctl")
            .args(["bootstrap", &session, plist_path.to_str().unwrap_or("")])
            .output();

        log::info!("LaunchAgent enabled: {} → {}", plist_path.display(), exe_str);
    } else {
        // Deregister from launchd/BTM first — prevents BTM from recreating the plist.
        let service = format!("{}/{}", session, LAUNCH_AGENT_LABEL);
        let _ = std::process::Command::new("launchctl")
            .args(["bootout", &service])
            .output();

        if plist_path.exists() {
            std::fs::remove_file(&plist_path).map_err(|e| e.to_string())?;
        }
        log::info!("LaunchAgent disabled: {}", plist_path.display());
    }

    Ok(get_launch_at_login_status())
}
