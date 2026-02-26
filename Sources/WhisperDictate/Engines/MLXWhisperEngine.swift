import Foundation

struct MLXWhisperEngine: ASREngine {
    let engineId = "mlx-whisper"
    let displayName = "Whisper MLX (Apple Silicon)"

    private static let hfCacheDir = "~/.cache/huggingface/hub"

    let models: [ASRModel] = [
        ASRModel(id: "mlx-whisper:tiny", engineId: "mlx-whisper", label: "Tiny",
                 filename: "models--mlx-community--whisper-tiny", url: "mlx-community/whisper-tiny",
                 size: "75 Mo", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "mlx-whisper:base", engineId: "mlx-whisper", label: "Base",
                 filename: "models--mlx-community--whisper-base-mlx", url: "mlx-community/whisper-base-mlx",
                 size: "145 Mo", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "mlx-whisper:small", engineId: "mlx-whisper", label: "Small",
                 filename: "models--mlx-community--whisper-small-mlx", url: "mlx-community/whisper-small-mlx",
                 size: "465 Mo", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "mlx-whisper:medium", engineId: "mlx-whisper", label: "Medium",
                 filename: "models--mlx-community--whisper-medium-mlx", url: "mlx-community/whisper-medium-mlx",
                 size: "1.5 Go", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "mlx-whisper:large-v3-turbo", engineId: "mlx-whisper", label: "Large V3 Turbo",
                 filename: "models--mlx-community--whisper-large-v3-turbo", url: "mlx-community/whisper-large-v3-turbo",
                 size: "1.6 Go", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "mlx-whisper:large-v3", engineId: "mlx-whisper", label: "Large V3",
                 filename: "models--mlx-community--whisper-large-v3-mlx", url: "mlx-community/whisper-large-v3-mlx",
                 size: "3.1 Go", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
        ASRModel(id: "mlx-whisper:distil-large-v3", engineId: "mlx-whisper", label: "Distil Large V3",
                 filename: "models--mlx-community--distil-whisper-large-v3", url: "mlx-community/distil-whisper-large-v3",
                 size: "1.5 Go", storageDir: hfCacheDir, downloadType: .huggingFaceRepo, downloadMarker: "refs/main"),
    ]

    let supportedLanguages = kCommonWhisperLanguages

    private let executablePath: String?

    init() {
        self.executablePath = findExecutable("mlx_whisper")
    }

    func resolveExecutable() -> String? { executablePath }

    func transcribe(model: ASRModel, audioURL: URL, language: String) throws -> String {
        guard let exe = executablePath else {
            throw TranscriberError.launchFailed(
                NSError(domain: "ASR", code: 1, userInfo: [NSLocalizedDescriptionKey: "mlx_whisper not found. Install: pip install mlx-whisper"])
            )
        }

        let tmpDir = NSTemporaryDirectory()
        var args = [audioURL.path, "--model", model.url, "--output-format", "txt",
                    "--output-dir", tmpDir, "--verbose", "False", "--condition-on-previous-text", "False"]
        if language != "auto" { args += ["--language", language] }

        let result = try ProcessRunner.run(executable: exe, arguments: args)

        let txtPath = tmpDir + audioURL.deletingPathExtension().lastPathComponent + ".txt"
        if FileManager.default.fileExists(atPath: txtPath) {
            let text = try String(contentsOfFile: txtPath, encoding: .utf8)
            try? FileManager.default.removeItem(atPath: txtPath)
            return text.trimmingCharacters(in: .whitespacesAndNewlines)
        }

        return result.stdout
    }
}
