//! Services for a platform with no implementation of its own. Permissions
//! report granted because nothing here can gate them.

use super::{PermissionReport, PermissionStatus};

impl Default for PermissionReport {
    fn default() -> Self {
        Self {
            microphone: PermissionStatus::Granted,
            accessibility: PermissionStatus::Granted,
            input_monitoring: PermissionStatus::Granted,
        }
    }
}

pub fn check_permissions() -> PermissionReport {
    PermissionReport::default()
}

pub fn request_permission(_kind: &str) -> bool {
    true
}

pub fn play_sound(_name: &str) {}

pub fn get_launch_at_login_status() -> &'static str {
    "disabled"
}

pub fn set_launch_at_login(_enabled: bool) -> Result<&'static str, String> {
    Err("Launch at login not supported on this platform".to_string())
}
