import Foundation

struct WhisperEngine: ASREngine {
    let engineId = "whisper"
    let displayName = "Whisper"

    let models: [ASRModel] = [
        ASRModel(
            id: "whisper:tiny", engineId: "whisper",
            label: "Tiny", filename: "ggml-tiny.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            size: "75 Mo", storageDir: "~/.local/share/whisper-cpp"
        ),
        ASRModel(
            id: "whisper:base", engineId: "whisper",
            label: "Base", filename: "ggml-base.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            size: "142 Mo", storageDir: "~/.local/share/whisper-cpp"
        ),
        ASRModel(
            id: "whisper:small", engineId: "whisper",
            label: "Small", filename: "ggml-small.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            size: "466 Mo", storageDir: "~/.local/share/whisper-cpp"
        ),
        ASRModel(
            id: "whisper:medium", engineId: "whisper",
            label: "Medium", filename: "ggml-medium.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
            size: "1.5 Go", storageDir: "~/.local/share/whisper-cpp"
        ),
        ASRModel(
            id: "whisper:large-v3-turbo", engineId: "whisper",
            label: "Large V3 Turbo", filename: "ggml-large-v3-turbo.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
            size: "1.6 Go", storageDir: "~/.local/share/whisper-cpp"
        ),
        ASRModel(
            id: "whisper:large-v3", engineId: "whisper",
            label: "Large V3", filename: "ggml-large-v3.bin",
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
            size: "3.1 Go", storageDir: "~/.local/share/whisper-cpp"
        ),
    ]

    let supportedLanguages: [(code: String, label: String)] = [
        ("auto", "Automatique"),
        ("fr", "Français"),
        ("en", "English"),
        ("es", "Español"),
        ("de", "Deutsch"),
    ]

    private let executablePath: String

    init() {
        let candidates = [
            "/opt/homebrew/bin/whisper-cli",
            "/usr/local/bin/whisper-cli",
        ]
        self.executablePath = candidates.first { FileManager.default.fileExists(atPath: $0) }
            ?? "whisper-cli"
    }

    func resolveExecutable() -> String? {
        FileManager.default.fileExists(atPath: executablePath) ? executablePath : nil
    }

    func transcribe(model: ASRModel, audioURL: URL, language: String) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executablePath)
        process.arguments = [
            "--model", model.localPath,
            "--language", language,
            "--no-timestamps",
            "--output-txt",
            "--file", audioURL.path,
        ]

        let pipe = Pipe()
        let errorPipe = Pipe()
        process.standardOutput = pipe
        process.standardError = errorPipe

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            throw TranscriberError.launchFailed(error)
        }

        let status = process.terminationStatus
        if status != 0 {
            let errorData = errorPipe.fileHandleForReading.readDataToEndOfFile()
            let errorStr = String(data: errorData, encoding: .utf8) ?? "Unknown error"
            Log.error("whisper-cli failed (status \(status)): \(errorStr)")
            throw TranscriberError.processFailed(status, errorStr)
        }

        let txtPath = audioURL.path + ".txt"
        if FileManager.default.fileExists(atPath: txtPath) {
            let text = try String(contentsOfFile: txtPath, encoding: .utf8)
            try? FileManager.default.removeItem(atPath: txtPath)
            return text.trimmingCharacters(in: .whitespacesAndNewlines)
        } else {
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            let text = String(data: data, encoding: .utf8) ?? ""
            return text.trimmingCharacters(in: .whitespacesAndNewlines)
        }
    }
}
