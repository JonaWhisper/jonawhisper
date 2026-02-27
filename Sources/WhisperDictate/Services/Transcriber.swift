import Foundation

class Transcriber {
    private let catalog = ASRModelCatalog.shared

    func transcribe(audioURL: URL) async throws -> String {
        let model = catalog.selectedModel
        guard model.isDownloaded else {
            throw TranscriberError.modelNotFound(model.localPath)
        }
        guard let engine = catalog.engine(for: model) else {
            throw TranscriberError.engineNotFound(model.engineId)
        }
        if engine.resolveExecutable() == nil && !model.isRemoteAPI {
            throw TranscriberError.engineUnavailable(engineId: engine.engineId, installHint: engine.installHint)
        }
        let language = catalog.selectedLanguage

        return try await Task.detached(priority: .userInitiated) {
            try engine.transcribe(model: model, audioURL: audioURL, language: language)
        }.value
    }
}
