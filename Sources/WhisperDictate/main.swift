import Cocoa

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate

// LSUIElement = true (set in Info.plist) hides Dock icon
app.setActivationPolicy(.accessory)

// Handle SIGTERM (pkill) — save download resume data before exit
signal(SIGTERM, SIG_IGN)
let sigSource = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .main)
sigSource.setEventHandler {
    ModelDownloader.shared.saveResumeDataAndCancel()
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { exit(0) }
}
sigSource.resume()

// Check and request all permissions, auto-restart if Input Monitoring is granted
PermissionChecker.requestAllAndVerify()

app.run()
