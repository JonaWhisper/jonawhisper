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

// MARK: - BadgeView

class BadgeView: NSView {
    override func draw(_ dirtyRect: NSRect) {
        let path = NSBezierPath(ovalIn: bounds)
        NSColor(red: 1.0, green: 0.3, blue: 0.3, alpha: 1.0).setFill()
        path.fill()
    }
}

// MARK: - PillView

class PillView: NSView {
    enum Mode {
        case recording
        case transcribing
        case downloading
    }

    var mode: Mode = .recording
    var spectrum: [Float] = []
    var downloadProgress: CGFloat = 0
    var downloadLabel: String = ""

    private var smoothedBars: [CGFloat] = Array(repeating: 0, count: 12)
    private var transcribingPhase: CGFloat = 0

    func resetSpectrum() {
        smoothedBars = Array(repeating: 0, count: 12)
    }

    override func draw(_ dirtyRect: NSRect) {
        let rect = bounds
        let path = NSBezierPath(roundedRect: rect, xRadius: rect.height / 2, yRadius: rect.height / 2)

        NSColor(white: 0.1, alpha: 0.9).setFill()
        path.fill()

        switch mode {
        case .recording:
            drawSpectrum(in: rect)
        case .transcribing:
            drawTranscribing(in: rect)
        case .downloading:
            drawDownloading(in: rect)
        }

        // Draw integrated progress line at bottom when downloading
        if downloadProgress > 0 {
            drawProgressLine(in: rect)
        }
    }

    private func drawProgressLine(in rect: NSRect) {
        NSGraphicsContext.current?.saveGraphicsState()

        // Clip to pill shape so the line respects rounded corners
        let clipPath = NSBezierPath(roundedRect: rect, xRadius: rect.height / 2, yRadius: rect.height / 2)
        clipPath.addClip()

        let lineHeight: CGFloat = 2
        let fullWidth = rect.width * downloadProgress
        let lineRect = NSRect(x: 0, y: 0, width: fullWidth, height: lineHeight)
        NSColor(white: 1.0, alpha: 0.7).setFill()
        NSBezierPath(rect: lineRect).fill()

        NSGraphicsContext.current?.restoreGraphicsState()
    }

    private func drawSpectrum(in rect: NSRect) {
        guard !spectrum.isEmpty else { return }

        let displayCount = 12
        var downsampled = [Float](repeating: 0, count: displayCount)
        for i in 0..<displayCount {
            let lo = i * spectrum.count / displayCount
            let hi = (i + 1) * spectrum.count / displayCount
            var sum: Float = 0
            for j in lo..<hi { sum += spectrum[j] }
            downsampled[i] = sum / Float(hi - lo)
        }

        // Rearrange: voice (low freq) in center, highs on edges
        var reordered = [Float](repeating: 0, count: displayCount)
        let mid = displayCount / 2
        for i in 0..<displayCount {
            let pos: Int
            if i == 0 {
                pos = mid
            } else if i % 2 == 1 {
                pos = mid + (i + 1) / 2
            } else {
                pos = mid - i / 2
            }
            if pos >= 0 && pos < displayCount {
                reordered[pos] = downsampled[i]
            }
        }

        let padding: CGFloat = 14
        let barWidth: CGFloat = 2
        let barSpacing: CGFloat = 2.5
        let totalWidth = CGFloat(displayCount) * (barWidth + barSpacing) - barSpacing
        let areaWidth = rect.width - padding * 2
        let startX = padding + (areaWidth - totalWidth) / 2
        let maxBarHeight = rect.height - 10
        let centerY = rect.midY

        for i in 0..<displayCount {
            let target = CGFloat(reordered[i])
            smoothedBars[i] = smoothedBars[i] * 0.45 + target * 0.55

            let barHeight = max(2, smoothedBars[i] * maxBarHeight)

            let x = startX + CGFloat(i) * (barWidth + barSpacing)
            let y = centerY - barHeight / 2

            let barRect = NSRect(x: x, y: y, width: barWidth, height: barHeight)
            let barPath = NSBezierPath(roundedRect: barRect, xRadius: barWidth / 2, yRadius: barWidth / 2)

            let alpha = 0.35 + 0.65 * smoothedBars[i]
            NSColor(white: 1.0, alpha: alpha).setFill()
            barPath.fill()
        }
    }

    private func drawDownloading(in rect: NSRect) {
        let padding: CGFloat = 6
        let barHeight: CGFloat = 4
        let barY = rect.midY - barHeight / 2
        let barWidth = rect.width - padding * 2

        // Background bar
        let bgRect = NSRect(x: padding, y: barY, width: barWidth, height: barHeight)
        let bgPath = NSBezierPath(roundedRect: bgRect, xRadius: barHeight / 2, yRadius: barHeight / 2)
        NSColor(white: 0.3, alpha: 1.0).setFill()
        bgPath.fill()

        // Progress bar
        let progressWidth = max(barHeight, barWidth * downloadProgress)
        let progressRect = NSRect(x: padding, y: barY, width: progressWidth, height: barHeight)
        let progressPath = NSBezierPath(roundedRect: progressRect, xRadius: barHeight / 2, yRadius: barHeight / 2)
        NSColor(white: 0.9, alpha: 1.0).setFill()
        progressPath.fill()
    }

    private func drawTranscribing(in rect: NSRect) {
        transcribingPhase += 0.12

        let dotCount = 3
        let dotRadius: CGFloat = 3
        let dotSpacing: CGFloat = 10
        let totalWidth = CGFloat(dotCount) * dotRadius * 2 + CGFloat(dotCount - 1) * dotSpacing
        let startX = (rect.width - totalWidth) / 2

        for i in 0..<dotCount {
            let phase = transcribingPhase - CGFloat(i) * 0.5
            let bounce = CGFloat(max(0, sin(phase))) * 4

            let x = startX + CGFloat(i) * (dotRadius * 2 + dotSpacing)
            let y = rect.midY - dotRadius + bounce

            let dotRect = NSRect(x: x, y: y, width: dotRadius * 2, height: dotRadius * 2)
            let dotPath = NSBezierPath(ovalIn: dotRect)

            NSColor(white: 0.9, alpha: 0.9).setFill()
            dotPath.fill()
        }
    }
}
