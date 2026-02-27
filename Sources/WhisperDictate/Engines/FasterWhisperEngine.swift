import Foundation

struct FasterWhisperEngine: ASREngine {
    let engineId = "faster-whisper"
    let displayName = "Whisper CTranslate2 (faster-whisper)"

    private static let hfCacheDir = "~/.cache/huggingface/hub"

    let models: [ASRModel] = [
        ASRModel(id: "faster-whisper:tiny", engineId: "faster-whisper", label: "Tiny",
                 filename: "models--Systran--faster-whisper-tiny", url: "Systran/faster-whisper-tiny",
                 size: "75 Mo", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "faster-whisper:base", engineId: "faster-whisper", label: "Base",
                 filename: "models--Systran--faster-whisper-base", url: "Systran/faster-whisper-base",
                 size: "145 Mo", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "faster-whisper:small", engineId: "faster-whisper", label: "Small",
                 filename: "models--Systran--faster-whisper-small", url: "Systran/faster-whisper-small",
                 size: "484 Mo", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "faster-whisper:medium", engineId: "faster-whisper", label: "Medium",
                 filename: "models--Systran--faster-whisper-medium", url: "Systran/faster-whisper-medium",
                 size: "1.5 Go", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "faster-whisper:large-v3-turbo", engineId: "faster-whisper", label: "Large V3 Turbo",
                 filename: "models--deepdml--faster-whisper-large-v3-turbo-ct2", url: "deepdml/faster-whisper-large-v3-turbo-ct2",
                 size: "1.6 Go", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "faster-whisper:large-v3", engineId: "faster-whisper", label: "Large V3",
                 filename: "models--Systran--faster-whisper-large-v3", url: "Systran/faster-whisper-large-v3",
                 size: "3.1 Go", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "faster-whisper:distil-large-v3", engineId: "faster-whisper", label: "Distil Large V3",
                 filename: "models--Systran--faster-distil-whisper-large-v3", url: "Systran/faster-distil-whisper-large-v3",
                 size: "1.5 Go", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
    ]

    let supportedLanguages = kCommonWhisperLanguages
    let installHint = "pip install whisper-ctranslate2"

    private let executablePath: String?

    init() {
        self.executablePath = findExecutable("whisper-ctranslate2")
    }

    func resolveExecutable() -> String? { executablePath }

    func transcribe(model: ASRModel, audioURL: URL, language: String) throws -> String {
        guard let exe = executablePath else {
            throw TranscriberError.launchFailed(
                NSError(domain: "ASR", code: 1, userInfo: [NSLocalizedDescriptionKey: "whisper-ctranslate2 not found. Install: pip install whisper-ctranslate2"])
            )
        }

        let tmpDir = NSTemporaryDirectory() + "faster-whisper-\(ProcessInfo.processInfo.processIdentifier)/"
        try? FileManager.default.createDirectory(atPath: tmpDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(atPath: tmpDir) }

        var args = [audioURL.path, "--model", model.url, "--output_format", "txt",
                    "--output_dir", tmpDir, "--compute_type", "int8", "--vad_filter", "true"]
        if language != "auto" { args += ["--language", language] }

        let result = try ProcessRunner.run(executable: exe, arguments: args)

        let txtPath = tmpDir + audioURL.deletingPathExtension().lastPathComponent + ".txt"
        if FileManager.default.fileExists(atPath: txtPath) {
            return try String(contentsOfFile: txtPath, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        }

        return result.stdout
    }
}
