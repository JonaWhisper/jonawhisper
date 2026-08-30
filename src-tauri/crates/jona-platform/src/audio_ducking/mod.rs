//! Lowering the system output volume while dictating, and putting it back.
//!
//! Both platforms move the control their volume keys drive, and both refuse to
//! restore a volume the user has moved since — that would undo their gesture.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{duck_volume, restore_volume};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{duck_volume, restore_volume};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn duck_volume(_reduction: f32) {}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn restore_volume() {}
