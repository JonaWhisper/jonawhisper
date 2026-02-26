import Cocoa

class FloatingPill: NSObject {
    private var window: NSWindow?
    private var pillView: PillView?
    private var badgeWindow: NSWindow?
    private var badgeLabel: NSTextField?
    private var spectrumProvider: (() -> [Float])?
    private var animTimer: Timer?
    private(set) var isDownloading = false

    var queueCount: Int = 0 {
        didSet {
            DispatchQueue.main.async {
                self.updateBadge()
            }
        }
    }

    func show(spectrumProvider: @escaping () -> [Float]) {
        self.spectrumProvider = spectrumProvider

        DispatchQueue.main.async {
            if self.window == nil {
                self.createWindow()
            }
            self.pillView?.mode = .recording
            self.pillView?.resetSpectrum()
            self.startTimer()
        }
    }

    func showTranscribing() {
        DispatchQueue.main.async {
            self.stopTimer()
            self.pillView?.mode = .transcribing
            self.pillView?.needsDisplay = true
            self.animTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { [weak self] timer in
                guard let self = self, let pill = self.pillView, pill.mode == .transcribing else {
                    timer.invalidate()
                    return
                }
                pill.needsDisplay = true
            }
        }
    }

    func showDownloading(_ modelName: String) {
        DispatchQueue.main.async {
            self.isDownloading = true
            if self.window == nil {
                self.createWindow()
            }
            // If not recording/transcribing, show download in pill
            if self.pillView?.mode != .recording && self.pillView?.mode != .transcribing {
                self.pillView?.mode = .downloading
                self.pillView?.downloadProgress = 0
                self.pillView?.downloadLabel = modelName
                self.pillView?.needsDisplay = true
            }
        }
    }

    func updateDownloadProgress(_ fraction: Double) {
        DispatchQueue.main.async {
            self.pillView?.downloadProgress = CGFloat(fraction)
            self.pillView?.needsDisplay = true
        }
    }

    func dismissDownload() {
        DispatchQueue.main.async {
            self.isDownloading = false
            self.pillView?.downloadProgress = 0
            self.pillView?.needsDisplay = true
        }
    }

    func dismiss() {
        DispatchQueue.main.async {
            self.stopTimer()
            self.isDownloading = false
            self.window?.animator().alphaValue = 0
            self.badgeWindow?.animator().alphaValue = 0
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                self.window?.orderOut(nil)
                self.window = nil
                self.pillView = nil
                self.badgeWindow?.orderOut(nil)
                self.badgeWindow = nil
                self.badgeLabel = nil
            }
        }
    }

    private func createWindow() {
        let pillWidth: CGFloat = 70
        let pillHeight: CGFloat = 28

        guard let screen = NSScreen.main else { return }
        let screenFrame = screen.frame
        let x = screenFrame.midX - pillWidth / 2
        let y = screenFrame.maxY - pillHeight - 40

        let frame = NSRect(x: x, y: y, width: pillWidth, height: pillHeight)

        let win = NSWindow(contentRect: frame, styleMask: .borderless, backing: .buffered, defer: false)
        win.level = .floating
        win.isOpaque = false
        win.backgroundColor = .clear
        win.hasShadow = true
        win.ignoresMouseEvents = true
        win.collectionBehavior = [.canJoinAllSpaces, .stationary]

        let pill = PillView(frame: NSRect(x: 0, y: 0, width: pillWidth, height: pillHeight))
        pill.mode = .recording
        win.contentView = pill

        win.alphaValue = 0
        win.orderFrontRegardless()
        win.animator().alphaValue = 1

        self.window = win
        self.pillView = pill

        // Create badge window (hidden by default)
        createBadgeWindow(pillFrame: frame)
    }

    private func createBadgeWindow(pillFrame: NSRect) {
        let badgeSize: CGFloat = 14
        let badgeX = pillFrame.maxX - badgeSize / 2 - 2
        let badgeY = pillFrame.maxY - badgeSize / 2 - 2

        let frame = NSRect(x: badgeX, y: badgeY, width: badgeSize, height: badgeSize)

        let win = NSWindow(contentRect: frame, styleMask: .borderless, backing: .buffered, defer: false)
        win.level = .floating
        win.isOpaque = false
        win.backgroundColor = .clear
        win.hasShadow = false
        win.ignoresMouseEvents = true
        win.collectionBehavior = [.canJoinAllSpaces, .stationary]

        let badge = BadgeView(frame: NSRect(x: 0, y: 0, width: badgeSize, height: badgeSize))
        win.contentView = badge

        let label = NSTextField(frame: NSRect(x: 0, y: 0, width: badgeSize, height: badgeSize - 1))
        label.isEditable = false
        label.isBordered = false
        label.drawsBackground = false
        label.alignment = .center
        label.font = NSFont.systemFont(ofSize: 8, weight: .bold)
        label.textColor = .white
        label.stringValue = ""
        badge.addSubview(label)

        win.alphaValue = 0
        self.badgeWindow = win
        self.badgeLabel = label
    }

    private func updateBadge() {
        if queueCount > 0 {
            badgeLabel?.stringValue = "\(queueCount)"
            badgeWindow?.orderFrontRegardless()
            badgeWindow?.animator().alphaValue = 1
        } else {
            badgeWindow?.animator().alphaValue = 0
        }
    }

    private func startTimer() {
        stopTimer()
        animTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30.0, repeats: true) { [weak self] timer in
            guard let self = self, let pill = self.pillView, pill.mode == .recording else {
                timer.invalidate()
                return
            }
            pill.spectrum = self.spectrumProvider?() ?? []
            pill.needsDisplay = true
        }
    }

    private func stopTimer() {
        animTimer?.invalidate()
        animTimer = nil
    }
}

