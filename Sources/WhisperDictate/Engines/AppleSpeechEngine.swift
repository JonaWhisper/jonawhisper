import Foundation
import Speech

struct AppleSpeechEngine: ASREngine {
    let engineId = "apple-speech"
    let displayName = "Apple Speech"

    let models: [ASRModel] = [
        ASRModel(id: "apple-speech:on-device", engineId: "apple-speech", label: "On-device",
                 filename: "", url: "", size: "Système", storageDir: "",
                 downloadType: .system),
    ]

    let supportedLanguages = kCommonWhisperLanguages
    let installHint = ""

    func resolveExecutable() -> String? {
        SFSpeechRecognizer()?.isAvailable == true ? "system" : nil
    }

    func transcribe(model: ASRModel, audioURL: URL, language: String) throws -> String {
        // Request authorization if needed
        if SFSpeechRecognizer.authorizationStatus() == .notDetermined {
            let sem = DispatchSemaphore(value: 0)
            SFSpeechRecognizer.requestAuthorization { _ in sem.signal() }
            sem.wait()
        }
        guard SFSpeechRecognizer.authorizationStatus() == .authorized else {
            throw TranscriberError.engineUnavailable(
                engineId: engineId,
                installHint: "Autorisez la reconnaissance vocale dans Réglages Système > Confidentialité > Reconnaissance vocale"
            )
        }

        let locale = language == "auto" ? Locale.current : Locale(identifier: language)
        guard let recognizer = SFSpeechRecognizer(locale: locale), recognizer.isAvailable else {
            throw TranscriberError.engineUnavailable(
                engineId: engineId,
                installHint: "Reconnaissance vocale non disponible pour \(locale.identifier)"
            )
        }

        let request = SFSpeechURLRecognitionRequest(url: audioURL)
        request.requiresOnDeviceRecognition = recognizer.supportsOnDeviceRecognition
        request.shouldReportPartialResults = false

        var resultText: String?
        var resultError: Error?
        let semaphore = DispatchSemaphore(value: 0)

        recognizer.recognitionTask(with: request) { result, error in
            if let result = result, result.isFinal {
                resultText = result.bestTranscription.formattedString
            }
            if let error = error {
                resultError = error
            }
            if result?.isFinal == true || error != nil {
                semaphore.signal()
            }
        }

        if semaphore.wait(timeout: .now() + 60) == .timedOut {
            throw TranscriberError.processFailed(0, "Timeout de la reconnaissance vocale")
        }

        if let error = resultError, resultText == nil {
            throw TranscriberError.launchFailed(error)
        }

        return resultText ?? ""
    }
}
