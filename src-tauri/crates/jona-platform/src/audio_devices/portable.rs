//! Device enumeration through cpal, for every platform without a native
//! implementation of its own.

use super::{AudioDevice, AudioTransportType};

/// cpal's WASAPI backend already reads the endpoint properties a native
/// implementation would go after — PKEY_AudioEndpoint_FormFactor,
/// JackSubType, EnumeratorName — so the transport comes from it rather than
/// from COM plumbing of our own.
fn transport_from(interface: cpal::InterfaceType) -> AudioTransportType {
    use cpal::InterfaceType as I;
    match interface {
        I::BuiltIn => AudioTransportType::BuiltIn,
        I::Usb => AudioTransportType::USB,
        I::Bluetooth => AudioTransportType::Bluetooth,
        I::Thunderbolt => AudioTransportType::Thunderbolt,
        I::Hdmi | I::DisplayPort => AudioTransportType::HDMI,
        I::Virtual => AudioTransportType::Virtual,
        I::Aggregate => AudioTransportType::Aggregate,
        // Pci, FireWire, Line, Spdif and Network have no counterpart in the
        // macOS-derived enum the shared UI matches on.
        _ => AudioTransportType::Unknown,
    }
}

/// Enumerate inputs through cpal rather than a native API: it already talks
/// to WASAPI, including the endpoint metadata. The one thing it does not
/// surface here is a stable identifier — `address()` stays empty on this
/// backend — so the name doubles as the uid the picker stores.
pub fn list_input_devices() -> Vec<AudioDevice> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.description().ok())
        .map(|desc| desc.name().to_string());

    let Ok(devices) = host.input_devices() else {
        log::warn!("Audio devices: cpal could not enumerate inputs");
        return vec![];
    };

    devices
        .enumerate()
        .filter_map(|(i, d)| {
            let desc = d.description().ok()?;
            let name = desc.name().to_string();
            Some(AudioDevice {
                id: i as u32,
                is_default: Some(&name) == default_name.as_ref(),
                uid: name.clone(),
                name,
                transport_type: transport_from(desc.interface_type()),
            })
        })
        .collect()
}

#[cfg(target_os = "windows")]
pub fn start_device_change_listener(callback: impl Fn() + Send + 'static) {
    crate::device_watcher::start(Box::new(callback));
}

/// Every other platform rebuilds the list when the panel opens, so a change
/// is picked up the next time the user looks.
#[cfg(not(target_os = "windows"))]
pub fn start_device_change_listener(_callback: impl Fn() + Send + 'static) {}
