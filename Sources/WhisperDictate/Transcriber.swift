import Foundation

class Transcriber {
    private let catalog = ASRModelCatalog.shared

    func transcribe(audioURL: URL, completion: @escaping (Result<String, Error>) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async { [self] in
            let model = catalog.selectedModel
            guard model.isDownloaded else {
                completion(.failure(TranscriberError.modelNotFound(model.localPath)))
                return
            }
            guard let engine = catalog.engine(for: model) else {
                completion(.failure(TranscriberError.engineNotFound(model.engineId)))
                return
            }
            do {
                let text = try engine.transcribe(model: model, audioURL: audioURL, language: catalog.selectedLanguage)
                completion(.success(text))
            } catch {
                completion(.failure(error))
            }
        }
    }
}

enum TranscriberError: LocalizedError {
    case modelNotFound(String)
    case engineNotFound(String)
    case launchFailed(Error)
    case processFailed(Int32, String)

    var errorDescription: String? {
        switch self {
        case .modelNotFound(let path):
            return "Model not found at \(path)"
        case .engineNotFound(let engineId):
            return "No engine found for \(engineId)"
        case .launchFailed(let error):
            return "Failed to launch: \(error.localizedDescription)"
        case .processFailed(let code, let msg):
            return "Process exited with code \(code): \(msg)"
        }
    }
}
