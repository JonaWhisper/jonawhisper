//! Centralised Tauri event name constants.
//! Use these instead of raw strings to avoid typos and enable refactoring.

// Recording lifecycle
pub const RECORDING_STARTED: &str = "recording-started";
pub const RECORDING_STOPPED: &str = "recording-stopped";
pub const SPECTRUM_DATA: &str = "spectrum-data";

// Transcription
pub const TRANSCRIPTION_STARTED: &str = "transcription-started";
pub const TRANSCRIPTION_COMPLETE: &str = "transcription-complete";
pub const TRANSCRIPTION_CANCELLED: &str = "transcription-cancelled";

// Pill window
pub const PILL_MODE: &str = "pill-mode";

// Model downloads
pub const DOWNLOAD_PROGRESS: &str = "download-progress";

// Settings & permissions
pub const SETTINGS_CHANGED: &str = "settings-changed";
pub const PERMISSION_CHANGED: &str = "permission-changed";

// Mic test
pub const MIC_TEST_SPECTRUM: &str = "mic-test-spectrum";
pub const MIC_TEST_STOPPED: &str = "mic-test-stopped";
