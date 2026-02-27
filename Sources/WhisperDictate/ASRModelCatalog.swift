import Foundation

class ASRModelCatalog {
    static let shared = ASRModelCatalog()

    private let localEngines: [ASREngine] = [
        WhisperEngine(),
        MLXWhisperEngine(),
        FasterWhisperEngine(),
        VoskEngine(),
        MoonshineEngine(),
    ]

    private var _cachedEngines: [ASREngine]?

    var engines: [ASREngine] {
        if let cached = _cachedEngines { return cached }
        let api = OpenAIAPIEngine()
        let result = api.models.isEmpty ? localEngines : localEngines + [api]
        _cachedEngines = result
        return result
    }

    /// Invalidate cached engines (call after API config changes)
    func invalidateEngines() {
        _cachedEngines = nil
    }

    var allModels: [ASRModel] { engines.flatMap { $0.models } }

    var sections: [(title: String, models: [ASRModel])] { engines.map { ($0.displayName, $0.models) } }

    func model(byId id: String) -> ASRModel? {
        allModels.first { $0.id == id }
    }

    func engine(for model: ASRModel) -> ASREngine? {
        engines.first { $0.engineId == model.engineId }
    }

    var downloadedModels: [ASRModel] {
        allModels.filter { $0.isDownloaded }
    }

    var supportedLanguages: [(code: String, label: String)] {
        var seen = Set<String>()
        var result: [(code: String, label: String)] = []
        for engine in engines {
            for lang in engine.supportedLanguages {
                if seen.insert(lang.code).inserted {
                    result.append(lang)
                }
            }
        }
        return result
    }

    private static let modelKey = "selectedASRModel"
    private static let languageKey = "whisperLanguage"

    var selectedModelId: String {
        get { UserDefaults.standard.string(forKey: Self.modelKey) ?? "whisper:large-v3-turbo" }
        set { UserDefaults.standard.set(newValue, forKey: Self.modelKey) }
    }

    var selectedModel: ASRModel {
        model(byId: selectedModelId)
            ?? model(byId: "whisper:large-v3-turbo")
            ?? allModels[0] // Whisper engine always provides models
    }

    var selectedLanguage: String {
        get { UserDefaults.standard.string(forKey: Self.languageKey) ?? "auto" }
        set { UserDefaults.standard.set(newValue, forKey: Self.languageKey) }
    }

    private init() {}
}
