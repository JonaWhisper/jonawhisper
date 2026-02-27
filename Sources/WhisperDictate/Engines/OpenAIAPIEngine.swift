import Foundation

// MARK: - Server configuration

struct APIServerConfig: Codable {
    var id: String
    var name: String        // "OpenAI", "Mon LocalAI"
    var baseURL: String     // "https://api.openai.com/v1" or "http://localhost:8080/v1"
    var apiKey: String      // API key (empty for local servers)
    var modelId: String     // "whisper-1" — sent in API request
    var modelLabel: String  // "Whisper 1" — displayed in UI

    init(name: String, baseURL: String, apiKey: String = "", modelId: String, modelLabel: String) {
        self.id = UUID().uuidString
        self.name = name
        self.baseURL = baseURL
        self.apiKey = apiKey
        self.modelId = modelId
        self.modelLabel = modelLabel
    }
}

// MARK: - Engine

struct OpenAIAPIEngine: ASREngine {
    let engineId = "openai-api"
    let displayName = "API"

    private static let configKey = "apiServers"

    var models: [ASRModel] {
        Self.loadConfigs().map { config in
            ASRModel(
                id: "api:\(config.id)",
                engineId: "openai-api",
                label: "\(config.modelLabel) — \(config.name)",
                filename: config.id,
                url: config.baseURL,
                size: "API",
                storageDir: "",
                downloadType: .remoteAPI
            )
        }
    }

    let supportedLanguages: [(code: String, label: String)] = [
        ("auto", "Automatique"),
        ("fr", "Français"),
        ("en", "English"),
        ("es", "Español"),
        ("de", "Deutsch"),
    ]

    let installHint = ""

    func resolveExecutable() -> String? { "http" }

    func transcribe(model: ASRModel, audioURL: URL, language: String) throws -> String {
        guard let config = Self.loadConfigs().first(where: { $0.id == model.filename }) else {
            throw TranscriberError.engineNotFound("api")
        }

        let base = config.baseURL.hasSuffix("/") ? String(config.baseURL.dropLast()) : config.baseURL
        let endpoint = base.hasSuffix("/v1")
            ? "\(base)/audio/transcriptions"
            : "\(base)/v1/audio/transcriptions"

        guard let url = URL(string: endpoint) else {
            throw TranscriberError.launchFailed(
                NSError(domain: "API", code: 1, userInfo: [NSLocalizedDescriptionKey: "URL invalide: \(endpoint)"])
            )
        }

        let boundary = "Boundary-\(UUID().uuidString)"
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("multipart/form-data; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
        request.timeoutInterval = 120
        if !config.apiKey.isEmpty {
            request.setValue("Bearer \(config.apiKey)", forHTTPHeaderField: "Authorization")
        }

        var body = Data()
        let audioData = try Data(contentsOf: audioURL)

        // file
        body.append("--\(boundary)\r\n")
        body.append("Content-Disposition: form-data; name=\"file\"; filename=\"\(audioURL.lastPathComponent)\"\r\n")
        body.append("Content-Type: audio/wav\r\n\r\n")
        body.append(audioData)
        body.append("\r\n")

        // model
        body.append("--\(boundary)\r\n")
        body.append("Content-Disposition: form-data; name=\"model\"\r\n\r\n")
        body.append("\(config.modelId)\r\n")

        // language
        if language != "auto" {
            body.append("--\(boundary)\r\n")
            body.append("Content-Disposition: form-data; name=\"language\"\r\n\r\n")
            body.append("\(language)\r\n")
        }

        // response_format
        body.append("--\(boundary)\r\n")
        body.append("Content-Disposition: form-data; name=\"response_format\"\r\n\r\n")
        body.append("text\r\n")

        body.append("--\(boundary)--\r\n")
        request.httpBody = body

        // Synchronous request (called from background queue)
        var responseData: Data?
        var responseError: Error?
        let semaphore = DispatchSemaphore(value: 0)

        URLSession.shared.dataTask(with: request) { data, response, error in
            responseData = data
            responseError = error
            semaphore.signal()
        }.resume()

        semaphore.wait()

        if let error = responseError {
            throw TranscriberError.launchFailed(error)
        }

        guard let data = responseData else {
            throw TranscriberError.processFailed(0, "Empty response from API")
        }

        // Check for error JSON response
        if let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let errorInfo = json["error"] as? [String: Any],
           let message = errorInfo["message"] as? String {
            throw TranscriberError.processFailed(0, message)
        }

        guard let text = String(data: data, encoding: .utf8) else {
            throw TranscriberError.processFailed(0, "Invalid response encoding")
        }

        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    // MARK: - Config persistence

    static func loadConfigs() -> [APIServerConfig] {
        guard let data = UserDefaults.standard.data(forKey: configKey) else { return [] }
        return (try? JSONDecoder().decode([APIServerConfig].self, from: data)) ?? []
    }

    static func saveConfigs(_ configs: [APIServerConfig]) {
        let data = try? JSONEncoder().encode(configs)
        UserDefaults.standard.set(data, forKey: configKey)
        ASRModelCatalog.shared.invalidateEngines()
    }

    static func addConfig(_ config: APIServerConfig) {
        var configs = loadConfigs()
        configs.append(config)
        saveConfigs(configs)
    }

    static func removeConfig(id: String) {
        var configs = loadConfigs()
        configs.removeAll { $0.id == id }
        saveConfigs(configs)
    }
}

// MARK: - Data helper

private extension Data {
    mutating func append(_ string: String) {
        if let data = string.data(using: .utf8) {
            append(data)
        }
    }
}
