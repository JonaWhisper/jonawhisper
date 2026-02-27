import Foundation

enum ASRDownloadType {
    case singleFile
    case huggingFaceRepo
    case zipArchive
    case command(executable: String, arguments: [String])
    case remoteAPI
    case system // Built-in, no download needed
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
        if case .system = downloadType { return true }
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
    var installHint: String { get }

    func resolveExecutable() -> String?
    func transcribe(model: ASRModel, audioURL: URL, language: String) throws -> String
}

// MARK: - Errors

enum TranscriberError: LocalizedError {
    case modelNotFound(String)
    case engineNotFound(String)
    case engineUnavailable(engineId: String, installHint: String)
    case launchFailed(Error)
    case processFailed(Int32, String)

    var errorDescription: String? {
        switch self {
        case .modelNotFound(let path): return "Model not found at \(path)"
        case .engineNotFound(let id): return "No engine found for \(id)"
        case .engineUnavailable(let id, _): return "Engine \(id) is not installed"
        case .launchFailed(let error): return "Failed to launch: \(error.localizedDescription)"
        case .processFailed(let code, let msg): return "Process exited with code \(code): \(msg)"
        }
    }

    /// User-facing notification message in French
    var userMessage: (title: String, body: String) {
        switch self {
        case .modelNotFound:
            return ("Modèle indisponible", "Le modèle n'est pas téléchargé. Ouvrez « Modèles… » pour le télécharger.")
        case .engineNotFound(let id):
            return ("Moteur introuvable", "Le moteur « \(id) » n'est pas enregistré.")
        case .engineUnavailable(_, let hint):
            return ("Moteur non installé", "Installez-le avec : \(hint)")
        case .launchFailed(let error):
            let desc = (error as NSError).localizedDescription
            return ("Erreur de lancement", desc)
        case .processFailed(_, let stderr):
            let msg = stderr.isEmpty ? "Le processus a échoué." : stderr
            let truncated = msg.count > 200 ? String(msg.prefix(200)) + "…" : msg
            return ("Erreur de transcription", truncated)
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
