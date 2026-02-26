import Foundation

class AppState {
    static let shared = AppState()
    private init() {}

    var isRecording = false
    var isTranscribing = false
    var transcriptionQueue: [URL] = []
    var downloadingModelId: String?
    var downloadProgress: Double = 0

    var isDownloading: Bool { downloadingModelId != nil }
    var queueCount: Int { transcriptionQueue.count }
    var isBusy: Bool { isRecording || isTranscribing || !transcriptionQueue.isEmpty || isDownloading }
}
