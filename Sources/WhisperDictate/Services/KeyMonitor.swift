import Cocoa

/// Available hotkey options for push-to-talk
struct HotkeyOption: Equatable {
    let keyCode: UInt16
    let flagMask: CGEventFlags
    let label: String

    static let rightCommand = HotkeyOption(keyCode: 0x36, flagMask: .maskCommand, label: "⌘ Commande droit")
    static let rightOption  = HotkeyOption(keyCode: 0x3D, flagMask: .maskAlternate, label: "⌥ Option droit")
    static let rightControl = HotkeyOption(keyCode: 0x3E, flagMask: .maskControl, label: "⌃ Contrôle droit")
    static let rightShift   = HotkeyOption(keyCode: 0x3C, flagMask: .maskShift, label: "⇧ Shift droit")

    static let all: [HotkeyOption] = [.rightCommand, .rightOption, .rightControl, .rightShift]

    private static let hotkeyKey = "hotkeyKeyCode"

    static var saved: HotkeyOption {
        let code = UserDefaults.standard.object(forKey: hotkeyKey) as? Int
        guard let code = code else { return .rightCommand }
        return all.first { Int($0.keyCode) == code } ?? .rightCommand
    }

    static func save(_ option: HotkeyOption) {
        UserDefaults.standard.set(Int(option.keyCode), forKey: hotkeyKey)
    }

    static func == (lhs: HotkeyOption, rhs: HotkeyOption) -> Bool {
        lhs.keyCode == rhs.keyCode
    }
}

/// Monitors a configurable modifier key via a CGEvent tap.
/// keyDown fires when the key is pressed alone,
/// keyUp fires when it's released (only if we were "active").
class KeyMonitor {
    private let onKeyDown: () -> Void
    private let onKeyUp: () -> Void
    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var retainedSelfPtr: UnsafeMutableRawPointer?
    private var keyHeld = false

    var hotkey: HotkeyOption

    init(onKeyDown: @escaping () -> Void, onKeyUp: @escaping () -> Void) {
        self.hotkey = HotkeyOption.saved
        self.onKeyDown = onKeyDown
        self.onKeyUp = onKeyUp
    }

    func start() {
        let eventMask: CGEventMask = (1 << CGEventType.flagsChanged.rawValue)

        let selfPtr = Unmanaged.passRetained(self).toOpaque()
        self.retainedSelfPtr = selfPtr

        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: eventMask,
            callback: KeyMonitor.eventCallback,
            userInfo: selfPtr
        ) else {
            Log.error("Failed to create event tap. Input Monitoring permission required.")
            DispatchQueue.main.async {
                let alert = NSAlert()
                alert.messageText = "Surveillance du clavier requise"
                alert.informativeText = "WhisperDictate a besoin de la permission « Surveillance de l'entrée » pour détecter la touche \(self.hotkey.label).\n\nAccordez l'accès dans Réglages Système > Confidentialité et sécurité > Surveillance de l'entrée, puis relancez l'app."
                alert.alertStyle = .warning
                alert.runModal()
            }
            return
        }

        self.eventTap = tap
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        self.runLoopSource = source
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)

        Log.info("Key monitor started (\(hotkey.label))")
    }

    func stop() {
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
        }
        if let source = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .commonModes)
        }
        if let ptr = retainedSelfPtr {
            Unmanaged<KeyMonitor>.fromOpaque(ptr).release()
            retainedSelfPtr = nil
        }
        eventTap = nil
        runLoopSource = nil
        keyHeld = false
    }

    /// Restart with a new hotkey
    func restart(with option: HotkeyOption) {
        stop()
        hotkey = option
        HotkeyOption.save(option)
        start()
    }

    /// C-compatible callback for CGEvent tap
    private static let eventCallback: CGEventTapCallBack = { proxy, type, event, userInfo in
        guard let userInfo = userInfo else { return Unmanaged.passUnretained(event) }
        let monitor = Unmanaged<KeyMonitor>.fromOpaque(userInfo).takeUnretainedValue()

        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            if let tap = monitor.eventTap {
                CGEvent.tapEnable(tap: tap, enable: true)
            }
            return Unmanaged.passUnretained(event)
        }

        guard type == .flagsChanged else {
            return Unmanaged.passUnretained(event)
        }

        let keyCode = event.getIntegerValueField(.keyboardEventKeycode)
        let flags = event.flags

        if keyCode == Int64(monitor.hotkey.keyCode) {
            if flags.contains(monitor.hotkey.flagMask) {
                if !monitor.keyHeld {
                    monitor.keyHeld = true
                    DispatchQueue.main.async {
                        monitor.onKeyDown()
                    }
                }
            } else {
                if monitor.keyHeld {
                    monitor.keyHeld = false
                    DispatchQueue.main.async {
                        monitor.onKeyUp()
                    }
                }
            }
        }

        return Unmanaged.passUnretained(event)
    }
}
