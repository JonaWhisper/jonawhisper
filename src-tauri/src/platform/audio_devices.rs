use std::ffi::c_void;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum AudioTransportType {
    BuiltIn,
    USB,
    Bluetooth,
    Virtual,
    Aggregate,
    Thunderbolt,
    HDMI,
    Unknown,
}

impl AudioTransportType {
    pub fn icon(&self) -> &'static str {
        match self {
            AudioTransportType::BuiltIn => "\u{1F4BB}",     // 💻
            AudioTransportType::USB => "\u{1F399}\u{FE0F}",  // 🎙️
            AudioTransportType::Bluetooth => "\u{1F3A7}",    // 🎧
            AudioTransportType::Virtual => "\u{1F30A}",      // 🌊
            AudioTransportType::Aggregate => "\u{1F4E6}",    // 📦
            AudioTransportType::Thunderbolt => "\u{26A1}",   // ⚡
            AudioTransportType::HDMI => "\u{1F4FA}",         // 📺
            AudioTransportType::Unknown => "\u{1F3A4}",      // 🎤
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioDevice {
    pub id: u32,
    pub name: String,
    pub uid: String,
    pub transport_type: AudioTransportType,
    pub is_default: bool,
}

// CoreAudio FFI — matches Apple C headers, hence non-snake_case
#[allow(non_upper_case_globals, non_snake_case, dead_code)]
mod ca {
    use std::ffi::c_void;

    pub type AudioObjectID = u32;
    pub type AudioDeviceID = u32;
    pub type OSStatus = i32;

    pub const kAudioObjectSystemObject: AudioObjectID = 1;

    // Property selectors (FourCC)
    pub const kAudioHardwarePropertyDevices: u32 = u32::from_be_bytes(*b"dev#");
    pub const kAudioHardwarePropertyDefaultInputDevice: u32 = u32::from_be_bytes(*b"dIn ");
    pub const kAudioDevicePropertyStreamConfiguration: u32 = u32::from_be_bytes(*b"slay");
    pub const kAudioDevicePropertyDeviceNameCFString: u32 = u32::from_be_bytes(*b"lnam");
    pub const kAudioDevicePropertyDeviceUID: u32 = u32::from_be_bytes(*b"uid ");
    pub const kAudioDevicePropertyTransportType: u32 = u32::from_be_bytes(*b"tran");

    // Scopes
    pub const kAudioObjectPropertyScopeGlobal: u32 = u32::from_be_bytes(*b"glob");
    pub const kAudioDevicePropertyScopeInput: u32 = u32::from_be_bytes(*b"inpt");
    pub const kAudioObjectPropertyElementMain: u32 = 0;

    // Transport types
    pub const kAudioDeviceTransportTypeBuiltIn: u32 = u32::from_be_bytes(*b"bltn");
    pub const kAudioDeviceTransportTypeUSB: u32 = u32::from_be_bytes(*b"usb ");
    pub const kAudioDeviceTransportTypeBluetooth: u32 = u32::from_be_bytes(*b"blue");
    pub const kAudioDeviceTransportTypeBluetoothLE: u32 = u32::from_be_bytes(*b"blea");
    pub const kAudioDeviceTransportTypeVirtual: u32 = u32::from_be_bytes(*b"virt");
    pub const kAudioDeviceTransportTypeAggregate: u32 = u32::from_be_bytes(*b"grup");
    pub const kAudioDeviceTransportTypeThunderbolt: u32 = u32::from_be_bytes(*b"thun");
    pub const kAudioDeviceTransportTypeHDMI: u32 = u32::from_be_bytes(*b"hdmi");
    pub const kAudioDeviceTransportTypeDisplayPort: u32 = u32::from_be_bytes(*b"dprt");

    #[repr(C)]
    pub struct AudioObjectPropertyAddress {
        pub mSelector: u32,
        pub mScope: u32,
        pub mElement: u32,
    }

    #[repr(C)]
    pub struct AudioBuffer {
        pub mNumberChannels: u32,
        pub mDataByteSize: u32,
        pub mData: *mut c_void,
    }

    #[repr(C)]
    pub struct AudioBufferList {
        pub mNumberBuffers: u32,
        pub mBuffers: [AudioBuffer; 1],
    }

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        pub fn AudioObjectGetPropertyDataSize(
            inObjectID: AudioObjectID,
            inAddress: *const AudioObjectPropertyAddress,
            inQualifierDataSize: u32,
            inQualifierData: *const c_void,
            outDataSize: *mut u32,
        ) -> OSStatus;

        pub fn AudioObjectGetPropertyData(
            inObjectID: AudioObjectID,
            inAddress: *const AudioObjectPropertyAddress,
            inQualifierDataSize: u32,
            inQualifierData: *const c_void,
            ioDataSize: *mut u32,
            outData: *mut c_void,
        ) -> OSStatus;

        pub fn AudioObjectAddPropertyListenerBlock(
            inObjectID: AudioObjectID,
            inAddress: *const AudioObjectPropertyAddress,
            inDispatchQueue: *mut c_void, // dispatch_queue_t
            inListener: *const c_void,    // block
        ) -> OSStatus;
    }
}

/// List all audio input devices with CoreAudio, including transport type and UID.
pub fn list_input_devices() -> Vec<AudioDevice> {
    let default_id = get_default_input_device_id();

    // SAFETY: CoreAudio property access via AudioObjectGetPropertyData(Size).
    // All property addresses use well-known Apple constants. Returned device IDs
    // are valid AudioObjectIDs for querying name, UID, and transport type.
    unsafe {
        // Get device count
        let mut size: u32 = 0;
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioHardwarePropertyDevices,
            mScope: ca::kAudioObjectPropertyScopeGlobal,
            mElement: ca::kAudioObjectPropertyElementMain,
        };

        if ca::AudioObjectGetPropertyDataSize(
            ca::kAudioObjectSystemObject, &address, 0, std::ptr::null(), &mut size,
        ) != 0 { return vec![]; }

        let count = size as usize / std::mem::size_of::<ca::AudioDeviceID>();
        let mut device_ids = vec![0u32; count];

        if ca::AudioObjectGetPropertyData(
            ca::kAudioObjectSystemObject, &address, 0, std::ptr::null(),
            &mut size, device_ids.as_mut_ptr() as *mut c_void,
        ) != 0 { return vec![]; }

        device_ids.into_iter().filter_map(|id| {
            // Check input channels
            if !has_input_channels(id) { return None; }

            let name = get_string_property(id, ca::kAudioDevicePropertyDeviceNameCFString)?;
            let uid = get_string_property(id, ca::kAudioDevicePropertyDeviceUID)?;
            let transport_type = get_transport_type(id);

            Some(AudioDevice {
                id,
                name,
                uid,
                transport_type,
                is_default: id == default_id,
            })
        }).collect()
    }
}

fn get_default_input_device_id() -> u32 {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioHardwarePropertyDefaultInputDevice,
            mScope: ca::kAudioObjectPropertyScopeGlobal,
            mElement: ca::kAudioObjectPropertyElementMain,
        };
        let mut device_id: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        ca::AudioObjectGetPropertyData(
            ca::kAudioObjectSystemObject, &address, 0, std::ptr::null(),
            &mut size, &mut device_id as *mut u32 as *mut c_void,
        );
        device_id
    }
}

/// SAFETY: device_id must be a valid AudioDeviceID from AudioObjectGetPropertyData.
unsafe fn has_input_channels(device_id: u32) -> bool {
    let address = ca::AudioObjectPropertyAddress {
        mSelector: ca::kAudioDevicePropertyStreamConfiguration,
        mScope: ca::kAudioDevicePropertyScopeInput,
        mElement: ca::kAudioObjectPropertyElementMain,
    };

    let mut size: u32 = 0;
    if ca::AudioObjectGetPropertyDataSize(device_id, &address, 0, std::ptr::null(), &mut size) != 0 {
        return false;
    }

    let buf = vec![0u8; size as usize];
    let buf_ptr = buf.as_ptr() as *mut c_void;
    if ca::AudioObjectGetPropertyData(device_id, &address, 0, std::ptr::null(), &mut size, buf_ptr) != 0 {
        return false;
    }

    let buffer_list = buf_ptr as *const ca::AudioBufferList;
    let n_buffers = (*buffer_list).mNumberBuffers as usize;
    let buffers_ptr = &(*buffer_list).mBuffers as *const ca::AudioBuffer;

    let mut total_channels = 0u32;
    for i in 0..n_buffers {
        total_channels += (*buffers_ptr.add(i)).mNumberChannels;
    }
    total_channels > 0
}

/// SAFETY: device_id must be a valid AudioDeviceID. selector must be a valid property selector
/// that returns a CFStringRef.
unsafe fn get_string_property(device_id: u32, selector: u32) -> Option<String> {
    let address = ca::AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: ca::kAudioObjectPropertyScopeGlobal,
        mElement: ca::kAudioObjectPropertyElementMain,
    };

    let mut cf_ref: *const c_void = std::ptr::null();
    let mut size = std::mem::size_of::<*const c_void>() as u32;

    if ca::AudioObjectGetPropertyData(
        device_id, &address, 0, std::ptr::null(),
        &mut size, &mut cf_ref as *mut *const c_void as *mut c_void,
    ) != 0 || cf_ref.is_null() {
        return None;
    }

    use core_foundation::base::TCFType;
    let cf_string = cf_ref as core_foundation::string::CFStringRef;
    let s = core_foundation::string::CFString::wrap_under_create_rule(cf_string);
    Some(s.to_string())
}

fn get_transport_type(device_id: u32) -> AudioTransportType {
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioDevicePropertyTransportType,
            mScope: ca::kAudioObjectPropertyScopeGlobal,
            mElement: ca::kAudioObjectPropertyElementMain,
        };

        let mut transport: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;

        if ca::AudioObjectGetPropertyData(
            device_id, &address, 0, std::ptr::null(),
            &mut size, &mut transport as *mut u32 as *mut c_void,
        ) != 0 {
            return AudioTransportType::Unknown;
        }

        match transport {
            ca::kAudioDeviceTransportTypeBuiltIn => AudioTransportType::BuiltIn,
            ca::kAudioDeviceTransportTypeUSB => AudioTransportType::USB,
            ca::kAudioDeviceTransportTypeBluetooth | ca::kAudioDeviceTransportTypeBluetoothLE => {
                AudioTransportType::Bluetooth
            }
            ca::kAudioDeviceTransportTypeVirtual => AudioTransportType::Virtual,
            ca::kAudioDeviceTransportTypeAggregate => AudioTransportType::Aggregate,
            ca::kAudioDeviceTransportTypeThunderbolt => AudioTransportType::Thunderbolt,
            ca::kAudioDeviceTransportTypeHDMI | ca::kAudioDeviceTransportTypeDisplayPort => {
                AudioTransportType::HDMI
            }
            _ => AudioTransportType::Unknown,
        }
    }
}

// Device change listener
static DEVICE_CHANGE_CALLBACK: Mutex<Option<Box<dyn Fn() + Send>>> = Mutex::new(None);

/// Start listening for audio device changes. Calls `callback` when devices are added/removed.
pub fn start_device_change_listener(callback: impl Fn() + Send + 'static) {
    *DEVICE_CHANGE_CALLBACK.lock().unwrap() = Some(Box::new(callback));

    // SAFETY: AudioObjectAddPropertyListener registers a C callback for device list changes.
    // The callback reads from the static DEVICE_CHANGE_CALLBACK mutex. The listener lives
    // for the process lifetime (CoreAudio retains it).
    unsafe {
        let address = ca::AudioObjectPropertyAddress {
            mSelector: ca::kAudioHardwarePropertyDevices,
            mScope: ca::kAudioObjectPropertyScopeGlobal,
            mElement: ca::kAudioObjectPropertyElementMain,
        };

        #[link(name = "CoreAudio", kind = "framework")]
        extern "C" {
            fn AudioObjectAddPropertyListener(
                inObjectID: u32,
                inAddress: *const ca::AudioObjectPropertyAddress,
                inListener: extern "C" fn(u32, u32, *const ca::AudioObjectPropertyAddress, *mut c_void) -> i32,
                inClientData: *mut c_void,
            ) -> i32;
        }

        extern "C" fn listener_proc(
            _id: u32, _count: u32, _addresses: *const ca::AudioObjectPropertyAddress,
            _client_data: *mut c_void,
        ) -> i32 {
            if let Some(ref cb) = *DEVICE_CHANGE_CALLBACK.lock().unwrap() {
                cb();
            }
            0
        }

        AudioObjectAddPropertyListener(
            ca::kAudioObjectSystemObject,
            &address,
            listener_proc,
            std::ptr::null_mut(),
        );
    }
}
