import Foundation

struct MoonshineEngine: ASREngine {
    let engineId = "moonshine"
    let displayName = "Moonshine"

    private static let cacheDir = "~/Library/Caches/moonshine_voice/download.moonshine.ai/model"

    let models: [ASRModel] = [
        ASRModel(id: "moonshine:tiny-en", engineId: "moonshine", label: "Tiny (EN)",
                 filename: "tiny-en/quantized/tiny-en",
                 url: "https://download.moonshine.ai/model/tiny-en/quantized/tiny-en",
                 size: "26 Mo", storageDir: cacheDir,
                 downloadType: .command(executable: "/usr/bin/env",
                    arguments: ["python3", "-m", "moonshine_voice.download", "--language", "en", "--model-arch", "0"]),
                 downloadMarker: "encoder_model.ort"),
        ASRModel(id: "moonshine:base-en", engineId: "moonshine", label: "Base (EN)",
                 filename: "base-en/quantized/base-en",
                 url: "https://download.moonshine.ai/model/base-en/quantized/base-en",
                 size: "134 Mo", storageDir: cacheDir,
                 downloadType: .command(executable: "/usr/bin/env",
                    arguments: ["python3", "-m", "moonshine_voice.download", "--language", "en", "--model-arch", "1"]),
                 downloadMarker: "encoder_model.ort"),
    ]

    let supportedLanguages: [(code: String, label: String)] = [
        ("en", "English"),
    ]

    let installHint = "pip install moonshine-voice"

    func resolveExecutable() -> String? {
        guard findExecutable("python3") != nil else { return nil }
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
        let result = try ProcessRunner.run(executable: "/usr/bin/env", arguments: [
            "python3", "-m", "moonshine_voice.transcriber",
            "--language", "en", "--wav-path", audioURL.path, "--quiet",
        ])

        return result.stdout.isEmpty ? result.stderr : result.stdout
    }
}
