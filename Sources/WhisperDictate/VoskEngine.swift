import Foundation

struct VoskEngine: ASREngine {
    let engineId = "vosk"
    let displayName = "Vosk"

    let models: [ASRModel] = [
        ASRModel(
            id: "vosk:small-en", engineId: "vosk",
            label: "Small English", filename: "vosk-model-small-en-us-0.15",
            url: "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip",
            size: "40 Mo", storageDir: "~/.cache/vosk",
            downloadType: .zipArchive
        ),
        ASRModel(
            id: "vosk:en", engineId: "vosk",
            label: "English", filename: "vosk-model-en-us-0.22",
            url: "https://alphacephei.com/vosk/models/vosk-model-en-us-0.22.zip",
            size: "1.8 Go", storageDir: "~/.cache/vosk",
            downloadType: .zipArchive
        ),
        ASRModel(
            id: "vosk:small-fr", engineId: "vosk",
            label: "Small Français", filename: "vosk-model-small-fr-0.22",
            url: "https://alphacephei.com/vosk/models/vosk-model-small-fr-0.22.zip",
            size: "41 Mo", storageDir: "~/.cache/vosk",
            downloadType: .zipArchive
        ),
        ASRModel(
            id: "vosk:fr", engineId: "vosk",
            label: "Français", filename: "vosk-model-fr-0.22",
            url: "https://alphacephei.com/vosk/models/vosk-model-fr-0.22.zip",
            size: "1.4 Go", storageDir: "~/.cache/vosk",
            downloadType: .zipArchive
        ),
    ]

    let supportedLanguages: [(code: String, label: String)] = [
        ("en-us", "English"),
        ("fr", "Français"),
    ]

    private let executablePath: String?

    init() {
        self.executablePath = findExecutable("vosk-transcriber")
    }

    func resolveExecutable() -> String? {
        executablePath
    }

    func transcribe(model: ASRModel, audioURL: URL, language: String) throws -> String {
        guard let exe = executablePath else {
            throw TranscriberError.launchFailed(
                NSError(domain: "ASR", code: 1, userInfo: [NSLocalizedDescriptionKey: "vosk-transcriber not found. Install: pip install vosk"])
            )
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: exe)
        process.arguments = [
            "-m", model.localPath,
            "-i", audioURL.path,
            "-t", "txt",
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
            Log.error("vosk-transcriber failed (status \(status)): \(errorStr)")
            throw TranscriberError.processFailed(status, errorStr)
        }

        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let text = String(data: data, encoding: .utf8) ?? ""
        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
