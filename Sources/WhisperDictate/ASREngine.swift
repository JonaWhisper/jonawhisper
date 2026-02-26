import Foundation

enum ASRDownloadType {
    case singleFile
    case huggingFaceRepo
    case zipArchive
    case command(executable: String, arguments: [String])
    case remoteAPI
}

struct ASRModel {
    let id: String          // "whisper:large-v3-turbo" (unique global)
    let engineId: String    // "whisper"
    let label: String       // "Large V3 Turbo"
    let filename: String    // "ggml-large-v3-turbo.bin" or directory name
    let url: String         // download URL, HF repo name, or zip URL
    let size: String        // "1.6 Go"
    let storageDir: String  // "~/.local/share/whisper-cpp"
    let downloadType: ASRDownloadType
    let downloadMarker: String? // relative file to check inside localPath for directory-based models

    init(id: String, engineId: String, label: String, filename: String, url: String,
         size: String, storageDir: String, downloadType: ASRDownloadType = .singleFile,
         downloadMarker: String? = nil) {
        self.id = id
        self.engineId = engineId
        self.label = label
        self.filename = filename
        self.url = url
        self.size = size
        self.storageDir = storageDir
        self.downloadType = downloadType
        self.downloadMarker = downloadMarker
    }

    var localPath: String {
        NSString(string: "\(storageDir)/\(filename)").expandingTildeInPath
    }

    var isDownloaded: Bool {
        if case .remoteAPI = downloadType { return true }
        if let marker = downloadMarker {
            return FileManager.default.fileExists(atPath: "\(localPath)/\(marker)")
        }
        return FileManager.default.fileExists(atPath: localPath)
    }

    var isRemoteAPI: Bool {
        if case .remoteAPI = downloadType { return true }
        return false
    }
}

protocol ASREngine {
    var engineId: String { get }
    var displayName: String { get }
    var models: [ASRModel] { get }
    var supportedLanguages: [(code: String, label: String)] { get }

    func resolveExecutable() -> String?
    func transcribe(model: ASRModel, audioURL: URL, language: String) throws -> String
}

// MARK: - Errors

enum TranscriberError: LocalizedError {
    case modelNotFound(String)
    case engineNotFound(String)
    case launchFailed(Error)
    case processFailed(Int32, String)

    var errorDescription: String? {
        switch self {
        case .modelNotFound(let path): return "Model not found at \(path)"
        case .engineNotFound(let id): return "No engine found for \(id)"
        case .launchFailed(let error): return "Failed to launch: \(error.localizedDescription)"
        case .processFailed(let code, let msg): return "Process exited with code \(code): \(msg)"
        }
    }
}

// MARK: - Common languages

let kCommonWhisperLanguages: [(code: String, label: String)] = [
    ("auto", "Automatique"),
    ("fr", "Français"),
    ("en", "English"),
    ("es", "Español"),
    ("de", "Deutsch"),
]

// MARK: - Executable lookup helper

func findExecutable(_ name: String, extraPaths: [String] = []) -> String? {
    let paths = extraPaths + ["/opt/homebrew/bin", "/usr/local/bin"]
    for dir in paths {
        let path = "\(dir)/\(name)"
        if FileManager.default.fileExists(atPath: path) {
            return path
        }
    }
    return nil
}
