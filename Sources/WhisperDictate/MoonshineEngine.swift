import Foundation

struct MoonshineEngine: ASREngine {
    let engineId = "moonshine"
    let displayName = "Moonshine"

    private static let cacheDir = "~/Library/Caches/moonshine_voice/download.moonshine.ai/model"

    let models: [ASRModel] = [
        ASRModel(
            id: "moonshine:tiny-en", engineId: "moonshine",
            label: "Tiny (EN)", filename: "tiny-en/quantized/tiny-en",
            url: "https://download.moonshine.ai/model/tiny-en/quantized/tiny-en",
            size: "26 Mo", storageDir: cacheDir,
            downloadType: .command(
                executable: "/usr/bin/env",
                arguments: ["python3", "-m", "moonshine_voice.download", "--language", "en", "--model-arch", "0"]
            ),
            downloadMarker: "encoder_model.ort"
        ),
        ASRModel(
            id: "moonshine:base-en", engineId: "moonshine",
            label: "Base (EN)", filename: "base-en/quantized/base-en",
            url: "https://download.moonshine.ai/model/base-en/quantized/base-en",
            size: "134 Mo", storageDir: cacheDir,
            downloadType: .command(
                executable: "/usr/bin/env",
                arguments: ["python3", "-m", "moonshine_voice.download", "--language", "en", "--model-arch", "1"]
            ),
            downloadMarker: "encoder_model.ort"
        ),
    ]

    let supportedLanguages: [(code: String, label: String)] = [
        ("en", "English"),
    ]

    init() {}

    func resolveExecutable() -> String? {
        // Moonshine uses python3 -m moonshine_voice.transcriber
        guard findExecutable("python3") != nil else { return nil }
        // Check if moonshine_voice is importable
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["python3", "-c", "import moonshine_voice"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
            process.waitUntilExit()
            return process.terminationStatus == 0 ? "/usr/bin/env" : nil
        } catch {
            return nil
        }
    }

    func transcribe(model: ASRModel, audioURL: URL, language: String) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = [
            "python3", "-m", "moonshine_voice.transcriber",
            "--language", "en",
            "--wav-path", audioURL.path,
            "--quiet",
        ]

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            throw TranscriberError.launchFailed(error)
        }

        let status = process.terminationStatus
        if status != 0 {
            let errorData = stderrPipe.fileHandleForReading.readDataToEndOfFile()
            let errorStr = String(data: errorData, encoding: .utf8) ?? "Unknown error"
            Log.error("moonshine failed (status \(status)): \(errorStr)")
            throw TranscriberError.processFailed(status, errorStr)
        }

        // Moonshine outputs to stdout or stderr depending on version
        let stdoutData = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
        let stderrData = stderrPipe.fileHandleForReading.readDataToEndOfFile()

        let stdout = String(data: stdoutData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let stderr = String(data: stderrData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

        return stdout.isEmpty ? stderr : stdout
    }
}
