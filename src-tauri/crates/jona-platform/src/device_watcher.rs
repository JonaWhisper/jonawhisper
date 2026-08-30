//! Audio device-change notifications through IMMNotificationClient.
//!
//! The COM object and the enumerator that holds its registration are neither
//! Send nor Sync, so they live on a thread of their own for as long as the app
//! runs; the callback reaches them through a static instead.

use std::sync::Mutex;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient,
    IMMNotificationClient_Impl, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows_core::{PCWSTR, Result, implement};

static CALLBACK: Mutex<Option<Box<dyn Fn() + Send>>> = Mutex::new(None);

fn notify() {
    let Ok(guard) = CALLBACK.lock() else { return };
    if let Some(callback) = guard.as_ref() {
        callback();
    }
}

#[implement(IMMNotificationClient)]
struct Watcher;

impl IMMNotificationClient_Impl for Watcher_Impl {
    fn OnDeviceStateChanged(&self, _id: &PCWSTR, _state: DEVICE_STATE) -> Result<()> {
        notify();
        Ok(())
    }

    fn OnDeviceAdded(&self, _id: &PCWSTR) -> Result<()> {
        notify();
        Ok(())
    }

    fn OnDeviceRemoved(&self, _id: &PCWSTR) -> Result<()> {
        notify();
        Ok(())
    }

    fn OnDefaultDeviceChanged(&self, _flow: EDataFlow, _role: ERole, _id: &PCWSTR) -> Result<()> {
        notify();
        Ok(())
    }

    /// Fires for every volume nudge and every property touch, which would have
    /// the picker rebuilding itself while the user drags a slider.
    fn OnPropertyValueChanged(&self, _id: &PCWSTR, _key: &PROPERTYKEY) -> Result<()> {
        Ok(())
    }
}

pub fn start(callback: Box<dyn Fn() + Send>) {
    *CALLBACK.lock().unwrap() = Some(callback);

    std::thread::spawn(|| unsafe {
        // MTA: notifications then arrive on a COM pool thread. An apartment
        // registration would deliver them through a message pump this thread
        // does not run.
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            log::warn!("device_watcher: CoInitializeEx failed, device changes go unnoticed");
            return;
        }
        let enumerator: IMMDeviceEnumerator =
            match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("device_watcher: no device enumerator: {e}");
                    return;
                }
            };

        let client: IMMNotificationClient = Watcher.into();
        if let Err(e) = enumerator.RegisterEndpointNotificationCallback(&client) {
            log::warn!("device_watcher: registration refused: {e}");
            return;
        }

        // The registration lasts exactly as long as these two objects. Parking
        // holds them without spinning; park can return spuriously, hence a loop.
        loop {
            std::thread::park();
        }
    });
}
