import Foundation

class ModelDownloader {
    static let shared = ModelDownloader()
    private init() {}

    private let lock = NSLock()

    // URLSession-based download state
    private var _activeTask: URLSessionDownloadTask?
    private var _activeSession: URLSession?
    private var _activeDelegate: DownloadDelegate?

    // Subprocess-based download state
    private var _activeProcess: Process?

    private var _activeModelId: String?

    var activeDownloadModelId: String? { lock.withLock { _activeModelId } }

    private static let pendingDir: String = {
        NSString(string: "~/.local/share/whisper-dictate").expandingTildeInPath
    }()

    private static func pendingDownloadPath() -> String {
        "\(pendingDir)/.pending-download"
    }

    private static func resumeDataPath(for model: ASRModel) -> String {
        let safeId = model.id.replacingOccurrences(of: ":", with: "-")
        let dir = NSString(string: model.storageDir).expandingTildeInPath
        return "\(dir)/.resume-\(safeId)"
    }

    func pendingDownloadModelId() -> String? {
        let path = Self.pendingDownloadPath()
        guard let data = FileManager.default.contents(atPath: path),
              let modelId = String(data: data, encoding: .utf8) else { return nil }
        guard let model = ASRModelCatalog.shared.model(byId: modelId) else {
            try? FileManager.default.removeItem(atPath: path)
            return nil
        }
        // Only require resume data for single file downloads
        if case .singleFile = model.downloadType {
            let resumePath = Self.resumeDataPath(for: model)
            guard FileManager.default.fileExists(atPath: resumePath) else {
                try? FileManager.default.removeItem(atPath: path)
                return nil
            }
        }
        return modelId
    }

    // MARK: - Download entry point

    func download(_ model: ASRModel, progress: @escaping (Double) -> Void) async -> Bool {
        try? FileManager.default.createDirectory(atPath: Self.pendingDir, withIntermediateDirectories: true)
        lock.withLock { _activeModelId = model.id }
        try? model.id.data(using: .utf8)?.write(to: URL(fileURLWithPath: Self.pendingDownloadPath()))

        if case .remoteAPI = model.downloadType { return true }
        if case .system = model.downloadType { return true }

        return await withCheckedContinuation { continuation in
            let complete: (Bool) -> Void = { success in
                continuation.resume(returning: success)
            }

            switch model.downloadType {
            case .singleFile:
                self.downloadWithURLSession(model, isZip: false, progress: progress, completion: complete)
            case .zipArchive:
                self.downloadWithURLSession(model, isZip: true, progress: progress, completion: complete)
            case .huggingFaceRepo:
                self.downloadWithSubprocess(
                    executable: "/usr/bin/env",
                    arguments: ["huggingface-cli", "download", model.url],
                    model: model, progress: progress, completion: complete
                )
            case .command(let executable, let arguments):
                self.downloadWithSubprocess(
                    executable: executable, arguments: arguments,
                    model: model, progress: progress, completion: complete
                )
            case .remoteAPI, .system:
                complete(true)
            }
        }
    }

    // MARK: - URLSession download (single file + zip)

    private func downloadWithURLSession(_ model: ASRModel, isZip: Bool, progress: @escaping (Double) -> Void, completion: @escaping (Bool) -> Void) {
        let storageDir = NSString(string: model.storageDir).expandingTildeInPath
        try? FileManager.default.createDirectory(atPath: storageDir, withIntermediateDirectories: true)

        let delegate = DownloadDelegate(progress: progress) { [weak self] location in
            guard let self = self else { return }
            self.clearPendingState(for: model)

            guard let location = location else {
                completion(false)
                return
            }

            do {
                if isZip {
                    let tmpZip = NSTemporaryDirectory() + UUID().uuidString + ".zip"
                    try FileManager.default.moveItem(at: location, to: URL(fileURLWithPath: tmpZip))

                    let unzip = Process()
                    unzip.executableURL = URL(fileURLWithPath: "/usr/bin/unzip")
                    unzip.arguments = ["-o", tmpZip, "-d", storageDir]
                    unzip.standardOutput = FileHandle.nullDevice
                    unzip.standardError = FileHandle.nullDevice
                    try unzip.run()
                    unzip.waitUntilExit()
                    try? FileManager.default.removeItem(atPath: tmpZip)

                    guard unzip.terminationStatus == 0 else {
                        Log.error("Failed to extract zip for model: \(model.id)")
                        completion(false)
                        return
                    }
                } else {
                    let dest = URL(fileURLWithPath: model.localPath)
                    if FileManager.default.fileExists(atPath: dest.path) {
                        try FileManager.default.removeItem(at: dest)
                    }
                    try FileManager.default.moveItem(at: location, to: dest)
                }
                Log.info("Downloaded model: \(model.id)")
                completion(true)
            } catch {
                Log.error("Failed to process model download: \(error)")
                completion(false)
            }
        }

        let session = URLSession(configuration: .default, delegate: delegate, delegateQueue: nil)
        lock.withLock {
            _activeDelegate = delegate
            _activeSession = session
        }

        // Resume data only for single file downloads
        if !isZip {
            let resumePath = Self.resumeDataPath(for: model)
            if let resumeData = FileManager.default.contents(atPath: resumePath) {
                Log.info("Resuming download for model: \(model.id)")
                let task = session.downloadTask(withResumeData: resumeData)
                lock.withLock { _activeTask = task }
                task.resume()
                return
            }
        }

        guard let url = URL(string: model.url) else {
            completion(false)
            return
        }
        Log.info("Starting download for model: \(model.id)")
        let task = session.downloadTask(with: url)
        lock.withLock { _activeTask = task }
        task.resume()
    }

    // MARK: - Subprocess download (HuggingFace, custom commands)

    private func downloadWithSubprocess(executable: String, arguments: [String], model: ASRModel, progress: @escaping (Double) -> Void, completion: @escaping (Bool) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

            let process = Process()
            process.executableURL = URL(fileURLWithPath: executable)
            process.arguments = arguments
            process.standardOutput = FileHandle.nullDevice

            // Capture stderr to parse tqdm progress
            let stderrPipe = Pipe()
            process.standardError = stderrPipe

            self.lock.withLock { self._activeProcess = process }

            // Read stderr in background and parse progress
            var lastReported = 0.0
            let readQueue = DispatchQueue(label: "download-stderr")
            readQueue.async {
                let handle = stderrPipe.fileHandleForReading
                var buffer = Data()
                while true {
                    let chunk = handle.availableData
                    if chunk.isEmpty { break }
                    buffer.append(chunk)

                    // Parse on \r or \n boundaries (tqdm uses \r for progress updates)
                    guard let text = String(data: buffer, encoding: .utf8) else { continue }
                    let lines = text.components(separatedBy: CharacterSet(charactersIn: "\r\n"))

                    // Keep the last incomplete fragment in buffer
                    if let last = lines.last {
                        buffer = last.data(using: .utf8) ?? Data()
                    }

                    // Parse percentage from completed lines
                    for line in lines.dropLast() {
                        if let pct = Self.parseProgress(line), pct > lastReported {
                            lastReported = pct
                            DispatchQueue.main.async { progress(pct) }
                        }
                    }
                }
            }

            do {
                try process.run()
                process.waitUntilExit()
            } catch {
                Log.error("Download subprocess failed: \(error)")
            }

            let success = process.terminationStatus == 0
            self.clearPendingState(for: model)

            if success {
                progress(1.0)
                Log.info("Downloaded model: \(model.id)")
            } else {
                Log.error("Download failed for model: \(model.id)")
            }
            completion(success)
        }
    }

    /// Parse tqdm-style progress output: "Fetching 5 files:  45%|████" → 0.45
    private static func parseProgress(_ line: String) -> Double? {
        // Match patterns like "45%" or "100%"
        guard let range = line.range(of: #"(\d+)%"#, options: .regularExpression) else { return nil }
        let match = line[range].dropLast() // remove the %
        guard let pct = Int(match) else { return nil }
        return Double(pct) / 100.0
    }

    // MARK: - Cancel / cleanup

    func saveResumeDataAndCancel() {
        let (task, modelId, process) = lock.withLock { (_activeTask, _activeModelId, _activeProcess) }
        if let task = task, let modelId = modelId,
           let model = ASRModelCatalog.shared.model(byId: modelId) {
            Log.info("Saving resume data for model: \(modelId)")
            task.cancel { resumeData in
                if let resumeData = resumeData {
                    let path = Self.resumeDataPath(for: model)
                    try? resumeData.write(to: URL(fileURLWithPath: path))
                    Log.info("Resume data saved (\(resumeData.count) bytes)")
                } else {
                    Log.info("No resume data available")
                }
            }
        } else if let process = process, process.isRunning {
            Log.info("Cancelling subprocess download")
            process.terminate()
        }
    }

    func cancelDownload() {
        let (task, modelId, process) = lock.withLock { (_activeTask, _activeModelId, _activeProcess) }
        guard let modelId = modelId,
              let model = ASRModelCatalog.shared.model(byId: modelId) else { return }
        task?.cancel()
        if let process = process, process.isRunning {
            process.terminate()
        }
        clearPendingState(for: model)
    }

    func deleteModel(_ model: ASRModel) -> Bool {
        guard model.isDownloaded else { return false }
        do {
            try FileManager.default.removeItem(atPath: model.localPath)
            return true
        } catch {
            Log.error("Failed to delete model \(model.id): \(error)")
            return false
        }
    }

    private func clearPendingState(for model: ASRModel) {
        try? FileManager.default.removeItem(atPath: Self.pendingDownloadPath())
        try? FileManager.default.removeItem(atPath: Self.resumeDataPath(for: model))
        lock.withLock {
            _activeTask = nil
            _activeSession = nil
            _activeDelegate = nil
            _activeProcess = nil
            _activeModelId = nil
        }
    }
}

// MARK: - Download delegate

private class DownloadDelegate: NSObject, URLSessionDownloadDelegate {
    let onProgress: (Double) -> Void
    let onComplete: (URL?) -> Void

    init(progress: @escaping (Double) -> Void, complete: @escaping (URL?) -> Void) {
        self.onProgress = progress
        self.onComplete = complete
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        onComplete(location)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                    didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        if totalBytesExpectedToWrite > 0 {
            onProgress(Double(totalBytesWritten) / Double(totalBytesExpectedToWrite))
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if error != nil {
            Log.error("Download failed: \(error!)")
            onComplete(nil)
        }
    }
}
