import CoreAudio
import Foundation

enum AudioTransportType: String {
    case builtIn = "builtin"
    case usb = "usb"
    case bluetooth = "bluetooth"
    case virtual = "virtual"
    case aggregate = "aggregate"
    case thunderbolt = "thunderbolt"
    case hdmi = "hdmi"
    case firewire = "firewire"
    case pci = "pci"
    case unknown = "unknown"

    /// SF Symbol name for the transport type
    var symbolName: String {
        switch self {
        case .builtIn: return "laptopcomputer"
        case .usb: return "cable.connector"
        case .bluetooth: return "headphones"
        case .virtual: return "waveform"
        case .aggregate: return "square.stack.3d.up"
        case .thunderbolt: return "bolt"
        case .hdmi: return "tv"
        case .firewire, .pci: return "cable.connector"
        case .unknown: return "mic"
        }
    }
}

struct AudioDevice: Equatable {
    let id: AudioDeviceID
    let name: String
    let uid: String
    let transportType: AudioTransportType

    static func == (lhs: AudioDevice, rhs: AudioDevice) -> Bool {
        lhs.id == rhs.id && lhs.uid == rhs.uid
    }
}

class AudioDeviceManager {
    /// Saved device UID preference key
    private static let preferenceKey = "selectedInputDeviceUID"

    /// List all available audio input devices
    static func listInputDevices() -> [AudioDevice] {
        var propertySize: UInt32 = 0
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )

        var status = AudioObjectGetPropertyDataSize(
            AudioObjectID(kAudioObjectSystemObject),
            &address, 0, nil, &propertySize
        )
        guard status == noErr else { return [] }

        let deviceCount = Int(propertySize) / MemoryLayout<AudioDeviceID>.size
        var deviceIDs = [AudioDeviceID](repeating: 0, count: deviceCount)

        status = AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject),
            &address, 0, nil, &propertySize, &deviceIDs
        )
        guard status == noErr else { return [] }

        return deviceIDs.compactMap { id -> AudioDevice? in
            // Check if device has input channels
            var inputAddress = AudioObjectPropertyAddress(
                mSelector: kAudioDevicePropertyStreamConfiguration,
                mScope: kAudioDevicePropertyScopeInput,
                mElement: kAudioObjectPropertyElementMain
            )

            var bufferListSize: UInt32 = 0
            guard AudioObjectGetPropertyDataSize(id, &inputAddress, 0, nil, &bufferListSize) == noErr else {
                return nil
            }

            let bufferListPtr = UnsafeMutablePointer<AudioBufferList>.allocate(capacity: 1)
            defer { bufferListPtr.deallocate() }

            guard AudioObjectGetPropertyData(id, &inputAddress, 0, nil, &bufferListSize, bufferListPtr) == noErr else {
                return nil
            }

            let inputChannels = UnsafeMutableAudioBufferListPointer(bufferListPtr).reduce(0) {
                $0 + Int($1.mNumberChannels)
            }
            guard inputChannels > 0 else { return nil }

            // Get device name
            var nameAddress = AudioObjectPropertyAddress(
                mSelector: kAudioDevicePropertyDeviceNameCFString,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain
            )
            var name: CFString = "" as CFString
            var nameSize = UInt32(MemoryLayout<CFString>.size)
            AudioObjectGetPropertyData(id, &nameAddress, 0, nil, &nameSize, &name)

            // Get device UID
            var uidAddress = AudioObjectPropertyAddress(
                mSelector: kAudioDevicePropertyDeviceUID,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain
            )
            var uid: CFString = "" as CFString
            var uidSize = UInt32(MemoryLayout<CFString>.size)
            AudioObjectGetPropertyData(id, &uidAddress, 0, nil, &uidSize, &uid)

            // Get transport type
            var transportAddress = AudioObjectPropertyAddress(
                mSelector: kAudioDevicePropertyTransportType,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain
            )
            var transportType: UInt32 = 0
            var transportSize = UInt32(MemoryLayout<UInt32>.size)
            AudioObjectGetPropertyData(id, &transportAddress, 0, nil, &transportSize, &transportType)

            let transport: AudioTransportType
            switch transportType {
            case kAudioDeviceTransportTypeBuiltIn:
                transport = .builtIn
            case kAudioDeviceTransportTypeUSB:
                transport = .usb
            case kAudioDeviceTransportTypeBluetooth, kAudioDeviceTransportTypeBluetoothLE:
                transport = .bluetooth
            case kAudioDeviceTransportTypeVirtual:
                transport = .virtual
            case kAudioDeviceTransportTypeAggregate:
                transport = .aggregate
            case kAudioDeviceTransportTypeThunderbolt:
                transport = .thunderbolt
            case kAudioDeviceTransportTypeHDMI, kAudioDeviceTransportTypeDisplayPort:
                transport = .hdmi
            case kAudioDeviceTransportTypeFireWire:
                transport = .firewire
            case kAudioDeviceTransportTypePCI:
                transport = .pci
            default:
                transport = .unknown
            }

            return AudioDevice(id: id, name: name as String, uid: uid as String, transportType: transport)
        }
    }

    /// Get the current system default input device
    static func getDefaultInputDevice() -> AudioDeviceID {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultInputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var deviceID: AudioDeviceID = 0
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject),
            &address, 0, nil, &size, &deviceID
        )
        return deviceID
    }

    /// Set the system default input device
    static func setDefaultInputDevice(_ deviceID: AudioDeviceID) {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultInputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var id = deviceID
        let size = UInt32(MemoryLayout<AudioDeviceID>.size)
        let status = AudioObjectSetPropertyData(
            AudioObjectID(kAudioObjectSystemObject),
            &address, 0, nil, size, &id
        )
        if status != noErr {
            Log.error("Failed to set default input device: \(status)")
        }
    }

    /// Save selected device UID to UserDefaults
    static func saveSelectedDevice(uid: String) {
        UserDefaults.standard.set(uid, forKey: preferenceKey)
        Log.info("Saved preferred mic: \(uid)")
    }

    /// Get saved device UID from UserDefaults
    static func getSavedDeviceUID() -> String? {
        UserDefaults.standard.string(forKey: preferenceKey)
    }

    /// Apply saved device preference (call at startup and before recording)
    static func applySavedDevice() {
        guard let savedUID = getSavedDeviceUID() else { return }
        let devices = listInputDevices()
        if let device = devices.first(where: { $0.uid == savedUID }) {
            setDefaultInputDevice(device.id)
            Log.info("Applied saved mic: \(device.name)")
        } else {
            Log.info("Saved mic not found (uid: \(savedUID)), using system default")
        }
    }
}
