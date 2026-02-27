import Cocoa

class PillView: NSView {
    enum Mode {
        case recording
        case transcribing
        case downloading
        case error
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
        case .error:
            drawError(in: rect)
        }

        if downloadProgress > 0 {
            drawProgressLine(in: rect)
        }
    }

    private func drawProgressLine(in rect: NSRect) {
        NSGraphicsContext.current?.saveGraphicsState()
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

        let bgRect = NSRect(x: padding, y: barY, width: barWidth, height: barHeight)
        let bgPath = NSBezierPath(roundedRect: bgRect, xRadius: barHeight / 2, yRadius: barHeight / 2)
        NSColor(white: 0.3, alpha: 1.0).setFill()
        bgPath.fill()

        let progressWidth = max(barHeight, barWidth * downloadProgress)
        let progressRect = NSRect(x: padding, y: barY, width: progressWidth, height: barHeight)
        let progressPath = NSBezierPath(roundedRect: progressRect, xRadius: barHeight / 2, yRadius: barHeight / 2)
        NSColor(white: 0.9, alpha: 1.0).setFill()
        progressPath.fill()
    }

    private func drawError(in rect: NSRect) {
        let size: CGFloat = 10
        let centerX = rect.midX
        let centerY = rect.midY
        let half = size / 2

        let path = NSBezierPath()
        path.lineWidth = 2.5
        path.lineCapStyle = .round

        path.move(to: NSPoint(x: centerX - half, y: centerY - half))
        path.line(to: NSPoint(x: centerX + half, y: centerY + half))
        path.move(to: NSPoint(x: centerX + half, y: centerY - half))
        path.line(to: NSPoint(x: centerX - half, y: centerY + half))

        NSColor(red: 1.0, green: 0.35, blue: 0.35, alpha: 0.9).setStroke()
        path.stroke()
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

// MARK: - BadgeView

class BadgeView: NSView {
    override func draw(_ dirtyRect: NSRect) {
        let path = NSBezierPath(ovalIn: bounds)
        NSColor(red: 1.0, green: 0.3, blue: 0.3, alpha: 1.0).setFill()
        path.fill()
    }
}
