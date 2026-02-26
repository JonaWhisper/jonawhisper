import Cocoa

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate

// LSUIElement = true (set in Info.plist) hides Dock icon
app.setActivationPolicy(.accessory)

// Handle SIGTERM (pkill) — save download resume data before exit
signal(SIGTERM) { _ in
    ModelDownloader.shared.saveResumeDataAndCancel()
    Thread.sleep(forTimeInterval: 0.3)
    exit(0)
}

// Check and request all permissions, auto-restart if Input Monitoring is granted
PermissionChecker.requestAllAndVerify()

app.run()
