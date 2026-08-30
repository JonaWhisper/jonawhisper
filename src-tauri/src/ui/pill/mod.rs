//! Native pill overlay window — no WebView, just an RGBA bitmap handed to each
//! platform's compositor: an NSImageView on macOS, a layered window on Windows.
//! Eliminates the WKWebView white flash entirely.

mod render;
use render::render_frame;
pub(crate) use render::PillFrame;

use super::overlay::Shared;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tauri::AppHandle;

const PILL_WIDTH: f64 = 80.0;
const PILL_HEIGHT: f64 = 32.0;
const PILL_TOP_OFFSET: f64 = 40.0;
/// Both backends animate at this rate.
const FRAME_INTERVAL: Duration = Duration::from_millis(33);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PillMode {
    Preparing,
    Recording,
    Paused,
    Transcribing,
    Success,
    Error,
    #[allow(dead_code)]
    Idle,
}

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod backend;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod backend;

/// Nothing to draw on with, so the overlay simply never appears.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod backend {
    use tauri::AppHandle;
    pub(super) fn open(_app: &AppHandle, _generation: u32) {}
    pub(super) fn close(_app: &AppHandle) {}
}

// -- Shared state --

/// Carries no window handle: each backend keeps its own, so the state the app
/// mutates — and the animation driven from it — is the same on every platform.
struct PillState {
    mode: PillMode,
    spectrum: [f32; 12],
    smoothed: [f32; 12],
    dot_phase: f32,
    pending_count: u32,
}

impl PillState {
    fn new(mode: PillMode) -> Self {
        Self { mode, spectrum: [0.0; 12], smoothed: [0.0; 12], dot_phase: 0.0, pending_count: 0 }
    }

    fn frame(&self) -> PillFrame {
        PillFrame {
            mode: self.mode,
            smoothed: self.smoothed,
            dot_phase: self.dot_phase,
            pending_count: self.pending_count,
        }
    }
}

static PILL: Shared<PillState> = Shared::new();

/// Advance one animation step. `None` once the pill is closed or superseded,
/// which is how both backends learn to tear their window down.
fn tick(generation: u32) -> Option<PillFrame> {
    static FRAMES: AtomicU32 = AtomicU32::new(0);
    static FLAT: AtomicU32 = AtomicU32::new(0);

    PILL.update(generation, |p| {
    p.dot_phase += 0.05;
    for i in 0..12 {
        p.smoothed[i] = p.smoothed[i] * 0.45 + p.spectrum[i] * 0.55;
    }

    let fc = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if p.mode == PillMode::Recording && fc.is_multiple_of(30) {
        let spec_max = p.spectrum.iter().cloned().fold(0.0f32, f32::max);
        let smooth_max = p.smoothed.iter().cloned().fold(0.0f32, f32::max);
        if smooth_max < 0.12 {
            let count = FLAT.fetch_add(30, Ordering::Relaxed) + 30;
            // Only warn after ~3s of sustained flat, then every ~3s
            if count >= 90 && count.is_multiple_of(90) {
                log::warn!("Pill render flat ({:.1}s): spec_max={:.4}, smooth_max={:.4}, spectrum={:.3?}",
                    count as f32 / 30.0, spec_max, smooth_max, p.spectrum);
            }
        } else {
            let prev = FLAT.swap(0, Ordering::Relaxed);
            if prev >= 90 {
                log::info!("Pill render recovered after {:.1}s flat", prev as f32 / 30.0);
            }
        }
    }
    p.frame()
    })
}

// -- Public API --

pub fn open(app: &AppHandle, initial_mode: PillMode) {
    let Some(generation) = PILL.open(PillState::new(initial_mode)) else {
        log::debug!("Pill: open() called but already open, skipping");
        return;
    };
    log::debug!("Pill: opening with mode {:?}", initial_mode);
    backend::open(app, generation);
}

pub fn close(app: &AppHandle) {
    if !PILL.close() {
        return;
    }
    log::debug!("Pill: closing");
    backend::close(app);
}

pub fn set_mode(mode: PillMode) {
    let applied = PILL.write(|p| {
        log::debug!("Pill: mode {:?} → {:?}", p.mode, mode);
    // Reset spectrum state when entering Recording to avoid stale smoothed values
    if mode == PillMode::Recording {
        let smooth_max = p.smoothed.iter().cloned().fold(0.0f32, f32::max);
        if smooth_max > 0.001 {
            log::debug!("Pill: resetting smoothed (was max={:.4})", smooth_max);
        }
            p.smoothed = [0.0; 12];
            p.spectrum = [0.0; 12];
        }
        p.mode = mode;
    });
    if applied.is_none() {
        log::warn!("Pill: set_mode({mode:?}) called but pill is not open");
    }
}

pub fn set_spectrum(data: &[f32]) {
    PILL.write(|p| {
        let n = data.len().min(12);
        p.spectrum[..n].copy_from_slice(&data[..n]);
    });
}

pub fn set_pending(count: u32) {
    PILL.write(|p| p.pending_count = count);
}

#[allow(dead_code)]
pub fn is_open() -> bool {
    PILL.is_open()
}

