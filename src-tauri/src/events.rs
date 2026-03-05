//! Centralised Tauri event name constants.
//! Use these instead of raw strings to avoid typos and enable refactoring.

// Recording lifecycle
pub const RECORDING_STARTED: &str = "recording-started";
pub const RECORDING_STOPPED: &str = "recording-stopped";
// Transcription
pub const TRANSCRIPTION_STARTED: &str = "transcription-started";
pub const TRANSCRIPTION_COMPLETE: &str = "transcription-complete";
pub const TRANSCRIPTION_CANCELLED: &str = "transcription-cancelled";

// Model downloads (now in jona_engines::downloader::DOWNLOAD_PROGRESS_EVENT)

// Settings & permissions
pub const SETTINGS_CHANGED: &str = "settings-changed";
pub const PERMISSION_CHANGED: &str = "permission-changed";

// Models
pub const MODELS_CHANGED: &str = "models-changed";

// Errors
pub const TRANSCRIPTION_ERROR: &str = "transcription-error";

// Shortcut capture
pub const SHORTCUT_CAPTURE_UPDATE: &str = "shortcut-capture-update";
pub const SHORTCUT_CAPTURE_COMPLETE: &str = "shortcut-capture-complete";

// Mic test
pub const MIC_TEST_SPECTRUM: &str = "mic-test-spectrum";
pub const MIC_TEST_STOPPED: &str = "mic-test-stopped";
