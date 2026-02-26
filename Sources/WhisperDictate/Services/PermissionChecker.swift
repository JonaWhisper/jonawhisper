import Cocoa
import AVFoundation
import UserNotifications

enum PermissionStatus: String {
    case granted = "✓"
    case denied = "✗"
    case undetermined = "?"
}

class PermissionChecker {

    struct PermissionReport {
        var microphone: PermissionStatus = .undetermined
        var accessibility: PermissionStatus = .undetermined
        var inputMonitoring: PermissionStatus = .undetermined
        var notifications: PermissionStatus = .undetermined

        var allGranted: Bool {
            microphone == .granted &&
            accessibility == .granted &&
            inputMonitoring == .granted &&
            notifications == .granted
        }

        func log() {
            Log.info("=== Permission Check ===")
            Log.info("  Microphone:       \(microphone.rawValue) \(microphone == .granted ? "" : "(System Settings > Privacy > Microphone)")")
            Log.info("  Accessibility:     \(accessibility.rawValue) \(accessibility == .granted ? "" : "(System Settings > Privacy > Accessibility)")")
            Log.info("  Input Monitoring:  \(inputMonitoring.rawValue) \(inputMonitoring == .granted ? "" : "(System Settings > Privacy > Input Monitoring)")")
            Log.info("  Notifications:     \(notifications.rawValue) \(notifications == .granted ? "" : "(System Settings > Privacy > Notifications)")")
            if allGranted {
                Log.info("=== All permissions OK ===")
            } else {
                Log.info("=== Some permissions missing ===")
            }
        }
    }

    /// Check all permissions without prompting
    static func check(completion: @escaping (PermissionReport) -> Void) {
        var report = PermissionReport()

        // Accessibility — synchronous check
        report.accessibility = AXIsProcessTrusted() ? .granted : .denied

        // Input Monitoring — test by trying to create an event tap
        report.inputMonitoring = checkInputMonitoring() ? .granted : .denied

        // Microphone — async
        let group = DispatchGroup()

        group.enter()
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            report.microphone = .granted
            group.leave()
        case .denied, .restricted:
            report.microphone = .denied
            group.leave()
        case .notDetermined:
            report.microphone = .undetermined
            group.leave()
        @unknown default:
            report.microphone = .undetermined
            group.leave()
        }

        // Notifications — async
        group.enter()
        UNUserNotificationCenter.current().getNotificationSettings { settings in
            switch settings.authorizationStatus {
            case .authorized, .provisional:
                report.notifications = .granted
            case .denied:
                report.notifications = .denied
            case .notDetermined:
                report.notifications = .undetermined
            @unknown default:
                report.notifications = .undetermined
            }
            group.leave()
        }

        group.notify(queue: .main) {
            completion(report)
        }
    }

    /// Request all missing permissions, then recheck and auto-restart if needed
    static func requestAllAndVerify() {
        check { initialReport in
            initialReport.log()

            if initialReport.allGranted {
                Log.info("All permissions already granted")
                return
            }

            let needsRestart = initialReport.inputMonitoring == .denied

            // Request microphone
            if initialReport.microphone != .granted {
                AVCaptureDevice.requestAccess(for: .audio) { granted in
                    Log.info("Microphone permission \(granted ? "granted" : "denied")")
                }
            }

            // Request notifications
            if initialReport.notifications != .granted {
                UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { granted, _ in
                    Log.info("Notification permission \(granted ? "granted" : "denied")")
                }
            }

            // Request accessibility (shows system prompt)
            if initialReport.accessibility != .granted {
                let opts = [kAXTrustedCheckOptionPrompt.takeRetainedValue(): true] as CFDictionary
                _ = AXIsProcessTrustedWithOptions(opts)
            }

            // Input Monitoring needs a restart after being granted
            if needsRestart {
                Log.info("Input Monitoring missing — will poll and restart when granted")
                pollInputMonitoringAndRestart()
            }
        }
    }

    /// Test input monitoring by creating a passive event tap
    private static func checkInputMonitoring() -> Bool {
        let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: (1 << CGEventType.flagsChanged.rawValue),
            callback: { _, _, event, _ in Unmanaged.passRetained(event) },
            userInfo: nil
        )
        if let tap = tap {
            // Clean up the test tap
            let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
            if let source = source {
                CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .commonModes)
                CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .commonModes)
            }
            CFMachPortInvalidate(tap)
            return true
        }
        return false
    }

    /// Poll every 2 seconds until Input Monitoring is granted, then restart
    private static func pollInputMonitoringAndRestart() {
        Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { timer in
            if checkInputMonitoring() {
                timer.invalidate()
                Log.info("Input Monitoring granted — restarting app")
                restartApp()
            }
        }
    }

    /// Restart the app
    private static func restartApp() {
        let bundlePath = Bundle.main.bundlePath
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        task.arguments = ["-n", bundlePath]
        try? task.run()

        // Quit current instance after a short delay
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            NSApplication.shared.terminate(nil)
        }
    }
}
