//! Windows platform services: permissions, system cues, launch at login.

use super::{PermissionReport, PermissionStatus};
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A registry string value, or None when the key or value is absent.
fn read_string(root: HKEY, path: &str, name: &str) -> Option<String> {
    let mut key: HKEY = std::ptr::null_mut();
    let opened =
        unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ, &mut key) };
    if opened != ERROR_SUCCESS {
        return None;
    }

    let name = wide(name);
    let mut size: u32 = 0;
    let measured = unsafe {
        RegQueryValueExW(key, name.as_ptr(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), &mut size)
    };
    if measured != ERROR_SUCCESS || size == 0 {
        unsafe { RegCloseKey(key) };
        return None;
    }

    let mut buffer = vec![0u8; size as usize];
    let read = unsafe {
        RegQueryValueExW(key, name.as_ptr(), std::ptr::null(), std::ptr::null_mut(), buffer.as_mut_ptr(), &mut size)
    };
    unsafe { RegCloseKey(key) };
    if read != ERROR_SUCCESS {
        return None;
    }

    let utf16: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&c| c != 0)
        .collect();
    Some(String::from_utf16_lossy(&utf16))
}

// -- Permissions --

/// Where Windows records the microphone consent the user set in Settings. The
/// per-app entries live under NonPackaged keyed by executable path, but the two
/// switches a user actually flips are these.
const MIC_CONSENT: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

/// Windows gates the microphone and nothing else here: a low-level keyboard
/// hook and SendInput need no consent, which is what accessibility and input
/// monitoring stand for on macOS.
pub fn check_permissions() -> PermissionReport {
    PermissionReport {
        microphone: microphone_status(),
        accessibility: PermissionStatus::Granted,
        input_monitoring: PermissionStatus::Granted,
    }
}

fn microphone_status() -> PermissionStatus {
    let denied = [MIC_CONSENT.to_string(), format!(r"{MIC_CONSENT}\NonPackaged")]
        .iter()
        .filter_map(|path| read_string(HKEY_CURRENT_USER, path, "Value"))
        .any(|value| value.eq_ignore_ascii_case("Deny"));

    // Absent means never restricted, which is the shipped default.
    if denied { PermissionStatus::Denied } else { PermissionStatus::Granted }
}

/// Windows has no consent dialog an app can raise; the switch lives in
/// Settings, so open the page on it.
pub fn request_permission(kind: &str) -> bool {
    if kind != "microphone" {
        return true;
    }
    let target = wide("ms-settings:privacy-microphone");
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns a value above 32 on success.
    (result as isize) > 32
}

// -- System cues --

/// The cues are named after the macOS sounds they were written for. Windows has
/// no equivalent library, so each maps to the system alias closest in intent.
fn alias_for(name: &str) -> &'static str {
    match name {
        "Basso" | "Funk" => "SystemHand",       // failure, cancellation
        "Glass" => "SystemNotification",        // transcription landed
        "Tink" => "SystemAsterisk",             // recording started
        "Pop" => "SystemDefault",               // recording stopped
        _ => "SystemDefault",
    }
}

pub fn play_sound(name: &str) {
    let alias = wide(alias_for(name));
    unsafe {
        PlaySoundW(alias.as_ptr(), std::ptr::null_mut(), SND_ALIAS | SND_ASYNC | SND_NODEFAULT)
    };
}

// -- Launch at login --

const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "JonaWhisper";

pub fn get_launch_at_login_status() -> &'static str {
    match read_string(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE) {
        Some(_) => "enabled",
        None => "disabled",
    }
}

/// Registers under HKCU\...\Run. No approval step exists here, unlike the
/// Login Items macOS routes SMAppService through, so the status is final.
pub fn set_launch_at_login(enabled: bool) -> Result<&'static str, String> {
    let mut key: HKEY = std::ptr::null_mut();
    let opened =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_WRITE, &mut key) };
    if opened != ERROR_SUCCESS {
        return Err(format!("cannot open the Run key (error {opened})"));
    }

    let name = wide(RUN_VALUE);
    let result = if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // Quoted: an unquoted path with a space is read as a command plus
        // arguments, and "C:\Program" is not an executable.
        let command = wide(&format!("\"{}\"", exe.display()));
        let bytes = command.len() * 2;
        unsafe {
            RegSetValueExW(key, name.as_ptr(), 0, REG_SZ, command.as_ptr().cast(), bytes as u32)
        }
    } else {
        unsafe { RegDeleteValueW(key, name.as_ptr()) }
    };
    unsafe { RegCloseKey(key) };

    if enabled && result != ERROR_SUCCESS {
        return Err(format!("cannot write the Run entry (error {result})"));
    }
    Ok(if enabled { "enabled" } else { "disabled" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cue_maps_to_an_alias() {
        for name in ["Basso", "Glass", "Tink", "Pop", "Funk"] {
            assert!(alias_for(name).starts_with("System"), "{name}");
        }
    }

    #[test]
    fn failure_and_success_do_not_share_a_cue() {
        assert_ne!(alias_for("Basso"), alias_for("Glass"));
        assert_ne!(alias_for("Tink"), alias_for("Pop"));
    }

    #[test]
    fn wide_strings_are_null_terminated() {
        assert_eq!(wide("ok"), vec![b'o' as u16, b'k' as u16, 0]);
        assert_eq!(wide(""), vec![0]);
    }
}
