import Cocoa

/// Monitors the right Command key via a CGEvent tap.
/// keyDown fires when right Command is pressed alone,
/// keyUp fires when it's released (only if we were "active").
class KeyMonitor {
    private let onKeyDown: () -> Void
    private let onKeyUp: () -> Void
    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var retainedSelfPtr: UnsafeMutableRawPointer?
    private var rightCmdHeld = false

    // Right Command keycode
    private static let kVK_RightCommand: UInt16 = 0x36

    init(onKeyDown: @escaping () -> Void, onKeyUp: @escaping () -> Void) {
        self.onKeyDown = onKeyDown
        self.onKeyUp = onKeyUp
    }

    func start() {
        let eventMask: CGEventMask = (1 << CGEventType.flagsChanged.rawValue)

        // Store self in a pointer for the C callback (released in stop())
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
            Log.error("Go to System Settings > Privacy & Security > Input Monitoring")

            // Prompt for input monitoring access
            // The system will show a prompt when we try to create the tap and fail
            DispatchQueue.main.async {
                let alert = NSAlert()
                alert.messageText = "Surveillance du clavier requise"
                alert.informativeText = "WhisperDictate a besoin de la permission « Surveillance de l'entrée » pour détecter la touche Commande droite.\n\nAccordez l'accès dans Réglages Système > Confidentialité et sécurité > Surveillance de l'entrée, puis relancez l'app."
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

        Log.info("Key monitor started (right Command key)")
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
    }

    /// C-compatible callback for CGEvent tap
    private static let eventCallback: CGEventTapCallBack = { proxy, type, event, userInfo in
        guard let userInfo = userInfo else { return Unmanaged.passUnretained(event) }
        let monitor = Unmanaged<KeyMonitor>.fromOpaque(userInfo).takeUnretainedValue()

        // If the tap gets disabled (e.g. system timeout), re-enable it
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

        if keyCode == Int64(kVK_RightCommand) {
            if flags.contains(.maskCommand) {
                // Right Command pressed
                if !monitor.rightCmdHeld {
                    monitor.rightCmdHeld = true
                    DispatchQueue.main.async {
                        monitor.onKeyDown()
                    }
                }
            } else {
                // Right Command released
                if monitor.rightCmdHeld {
                    monitor.rightCmdHeld = false
                    DispatchQueue.main.async {
                        monitor.onKeyUp()
                    }
                }
            }
        } else {
            // Another modifier was pressed while right cmd is held — cancel
            if monitor.rightCmdHeld {
                // Don't cancel, just let it pass through
            }
        }

        return Unmanaged.passUnretained(event)
    }
}
