import Foundation

class AppState {
    static let shared = AppState()
    private init() {}

    private let lock = NSLock()

    private var _isRecording = false
    private var _isTranscribing = false
    private var _transcriptionQueue: [URL] = []
    private var _downloadingModelId: String?
    private var _downloadProgress: Double = 0
    private var _transcriptionCancelled = false

    var isRecording: Bool {
        get { lock.withLock { _isRecording } }
        set { lock.withLock { _isRecording = newValue } }
    }

    var isTranscribing: Bool {
        get { lock.withLock { _isTranscribing } }
        set { lock.withLock { _isTranscribing = newValue } }
    }

    var transcriptionQueue: [URL] {
        get { lock.withLock { _transcriptionQueue } }
        set { lock.withLock { _transcriptionQueue = newValue } }
    }

    var downloadingModelId: String? {
        get { lock.withLock { _downloadingModelId } }
        set { lock.withLock { _downloadingModelId = newValue } }
    }

    var downloadProgress: Double {
        get { lock.withLock { _downloadProgress } }
        set { lock.withLock { _downloadProgress = newValue } }
    }

    var transcriptionCancelled: Bool {
        get { lock.withLock { _transcriptionCancelled } }
        set { lock.withLock { _transcriptionCancelled = newValue } }
    }

    var isDownloading: Bool { downloadingModelId != nil }
    var queueCount: Int { transcriptionQueue.count }
    var isBusy: Bool { isRecording || isTranscribing || !transcriptionQueue.isEmpty || isDownloading }

    func enqueue(_ url: URL) -> Int {
        lock.withLock {
            _transcriptionQueue.append(url)
            return _transcriptionQueue.count
        }
    }

    func dequeue() -> URL? {
        lock.withLock {
            _transcriptionQueue.isEmpty ? nil : _transcriptionQueue.removeFirst()
        }
    }
}
