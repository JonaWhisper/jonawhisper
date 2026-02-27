import Cocoa

extension Notification.Name {
    static let modelDownloadCompleted = Notification.Name("modelDownloadCompleted")
}

class ModelManagerWindowController: NSWindowController {
    static let shared = ModelManagerWindowController()
    weak var pill: FloatingPill?

    private var engineTableView: NSTableView?
    private var modelTableView: NSTableView?
    private var state: AppState { AppState.shared }

    private var displayEngines: [ASREngine] = []
    private var engineAvailable: [String: Bool] = [:]  // engineId -> is CLI installed
    private var keyMonitor: Any?

    private enum ModelRow {
        case model(ASRModel)
        case addAPIServer
        case engineUnavailable(installHint: String)
    }
    private var modelRows: [ModelRow] = []

    private init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 660, height: 420),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "Modèles"
        window.center()
        window.isReleasedWhenClosed = false

        super.init(window: window)
        rebuildEngineList()
        setupUI()

        // Select engine containing the currently selected model
        let selectedId = ASRModelCatalog.shared.selectedModelId
        let initialIndex = displayEngines.firstIndex { $0.models.contains { $0.id == selectedId } } ?? 0
        engineTableView?.selectRowIndexes(IndexSet(integer: initialIndex), byExtendingSelection: false)
        selectEngine(at: initialIndex)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        if let monitor = keyMonitor { NSEvent.removeMonitor(monitor) }
    }

    static func showWindow() {
        shared.showWindow(nil)
        shared.window?.makeKeyAndOrderFront(nil)
        shared.rebuildEngineList()
        shared.engineTableView?.reloadData()
        let idx = max(0, shared.engineTableView?.selectedRow ?? 0)
        if idx < shared.displayEngines.count {
            shared.engineTableView?.selectRowIndexes(IndexSet(integer: idx), byExtendingSelection: false)
        }
        shared.selectEngine(at: idx)
        NSApp.activate(ignoringOtherApps: true)
    }

    // MARK: - Data

    private func rebuildEngineList() {
        displayEngines = ASRModelCatalog.shared.engines
        if !displayEngines.contains(where: { $0.engineId == "openai-api" }) {
            displayEngines.append(OpenAIAPIEngine())
        }
        engineAvailable = [:]
        for engine in displayEngines {
            engineAvailable[engine.engineId] = engine.resolveExecutable() != nil
        }
    }

    private func selectEngine(at index: Int) {
        guard index >= 0 && index < displayEngines.count else { return }
        let engine = displayEngines[index]
        let available = engineAvailable[engine.engineId] ?? false

        if engine.engineId == "openai-api" {
            modelRows = engine.models.map { .model($0) }
            modelRows.append(.addAPIServer)
        } else if available {
            modelRows = engine.models.map { .model($0) }
        } else {
            modelRows = [.engineUnavailable(installHint: engine.installHint)]
            modelRows += engine.models.map { .model($0) }
        }
        modelTableView?.reloadData()
    }

    private func toolSubtitle(for engine: ASREngine) -> String {
        switch engine.engineId {
        case "whisper": return "whisper-cli"
        case "mlx-whisper": return "mlx_whisper"
        case "faster-whisper": return "whisper-ctranslate2"
        case "vosk": return "vosk-transcriber"
        case "moonshine": return "moonshine_voice"
        case "apple-speech": return "SFSpeechRecognizer"
        case "openai-api": return "HTTP API"
        default: return engine.engineId
        }
    }

    // MARK: - UI Setup

    private func setupUI() {
        guard let contentView = window?.contentView else { return }

        let splitView = NSSplitView(frame: contentView.bounds)
        splitView.isVertical = true
        splitView.dividerStyle = .thin
        splitView.autoresizingMask = [.width, .height]
        splitView.delegate = self

        // Left: Engine sidebar
        let leftScroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 185, height: 420))
        leftScroll.autoresizingMask = [.width, .height]
        leftScroll.hasVerticalScroller = true
        leftScroll.borderType = .noBorder

        let engineTable = NSTableView()
        engineTable.style = .sourceList
        engineTable.headerView = nil
        engineTable.allowsEmptySelection = false
        let engineCol = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("engine"))
        engineCol.resizingMask = .autoresizingMask
        engineTable.addTableColumn(engineCol)
        engineTable.dataSource = self
        engineTable.delegate = self
        leftScroll.documentView = engineTable
        self.engineTableView = engineTable

        // Right: Model list
        let rightScroll = NSScrollView(frame: NSRect(x: 186, y: 0, width: 474, height: 420))
        rightScroll.autoresizingMask = [.width, .height]
        rightScroll.hasVerticalScroller = true
        rightScroll.borderType = .noBorder

        let modelTable = NSTableView()
        modelTable.style = .fullWidth
        modelTable.headerView = nil
        modelTable.selectionHighlightStyle = .regular
        modelTable.intercellSpacing = NSSize(width: 0, height: 1)
        let modelCol = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("model"))
        modelCol.resizingMask = .autoresizingMask
        modelTable.addTableColumn(modelCol)
        modelTable.dataSource = self
        modelTable.delegate = self
        rightScroll.documentView = modelTable
        self.modelTableView = modelTable

        splitView.addSubview(leftScroll)
        splitView.addSubview(rightScroll)
        contentView.addSubview(splitView)

        splitView.setPosition(185, ofDividerAt: 0)
        splitView.adjustSubviews()

        // Keyboard shortcuts
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self = self, event.window === self.window else { return event }
            return self.handleKeyDown(event) ? nil : event
        }
    }

    private func handleKeyDown(_ event: NSEvent) -> Bool {
        // Cmd+N → add API server
        if event.modifierFlags.contains(.command) && event.charactersIgnoringModifiers == "n" {
            showAPIConfigSheet(existing: nil)
            return true
        }

        // Cmd+W → close window
        if event.modifierFlags.contains(.command) && event.charactersIgnoringModifiers == "w" {
            window?.close()
            return true
        }

        guard !event.modifierFlags.contains(.command) else { return false }

        let focusedOnModelTable = window?.firstResponder === modelTableView

        switch event.keyCode {
        case 36: // Enter
            if focusedOnModelTable {
                return handleEnterOnModel()
            }
        case 51: // Delete/Backspace
            if focusedOnModelTable {
                return handleDeleteOnModel()
            }
        case 48: // Tab → switch focus between engine/model tables
            if focusedOnModelTable {
                window?.makeFirstResponder(engineTableView)
            } else {
                window?.makeFirstResponder(modelTableView)
            }
            return true
        default:
            break
        }

        return false
    }

    private func handleEnterOnModel() -> Bool {
        guard let row = modelTableView?.selectedRow, row >= 0,
              let model = modelFromRow(row) else { return false }

        if model.isDownloaded {
            ASRModelCatalog.shared.selectedModelId = model.id
            modelTableView?.reloadData()
        } else if state.downloadingModelId == nil {
            let available = engineAvailable[model.engineId] ?? false
            if available { startDownload(model) }
        }
        return true
    }

    private func handleDeleteOnModel() -> Bool {
        guard let row = modelTableView?.selectedRow, row >= 0,
              modelFromRow(row) != nil else { return false }

        let fakeButton = NSButton()
        fakeButton.tag = row
        deleteClicked(fakeButton)
        return true
    }

    // MARK: - Download

    func startDownload(_ model: ASRModel) {
        Log.info("startDownload: \(model.id), pill=\(pill != nil)")
        state.downloadingModelId = model.id
        state.downloadProgress = 0
        pill?.showDownloading(model.label)
        modelTableView?.reloadData()

        Task { @MainActor [weak self] in
            let success = await ModelDownloader.shared.download(model) { fraction in
                DispatchQueue.main.async {
                    AppState.shared.downloadProgress = fraction
                    self?.pill?.updateDownloadProgress(fraction)
                    self?.updateProgressRow(for: model.id)
                }
            }

            guard let self = self else { return }
            AppState.shared.downloadingModelId = nil
            AppState.shared.downloadProgress = 0
            self.pill?.dismissDownload()

            if success {
                ASRModelCatalog.shared.selectedModelId = model.id
                Log.info("Model \(model.id) downloaded and selected")
                NSSound(named: "Glass")?.play()
                NotificationService.show(title: "Modèle prêt", body: "\(model.label) est téléchargé et sélectionné.")
            } else {
                Log.error("Failed to download model: \(model.id)")
                NSSound(named: "Basso")?.play()
                NotificationService.show(title: "Échec du téléchargement", body: "Le modèle \(model.label) n'a pas pu être téléchargé. Vérifiez votre connexion.")
            }

            let idx = max(0, self.engineTableView?.selectedRow ?? 0)
            self.selectEngine(at: idx)
            NotificationCenter.default.post(name: .modelDownloadCompleted, object: nil)
        }
    }

    private func updateProgressRow(for modelId: String) {
        guard let index = modelRows.firstIndex(where: {
            if case .model(let m) = $0 { return m.id == modelId }
            return false
        }) else { return }
        if let cell = modelTableView?.view(atColumn: 0, row: index, makeIfNecessary: false) as? ModelCellView {
            cell.updateProgress(state.downloadProgress)
        }
    }

    private func modelFromRow(_ row: Int) -> ASRModel? {
        guard row >= 0 && row < modelRows.count, case .model(let m) = modelRows[row] else { return nil }
        return m
    }

    // MARK: - Actions

    @objc func radioClicked(_ sender: NSButton) {
        guard let model = modelFromRow(sender.tag), model.isDownloaded else { return }
        ASRModelCatalog.shared.selectedModelId = model.id
        modelTableView?.reloadData()
    }

    @objc func downloadClicked(_ sender: NSButton) {
        guard let model = modelFromRow(sender.tag) else { return }
        Log.info("Download clicked: \(model.id) (row \(sender.tag))")
        startDownload(model)
    }

    @objc func deleteClicked(_ sender: NSButton) {
        guard let model = modelFromRow(sender.tag) else { return }

        if model.isRemoteAPI {
            let configId = model.filename
            let alert = NSAlert()
            alert.messageText = "Supprimer le serveur \(model.label) ?"
            alert.informativeText = "La configuration de ce serveur API sera supprimée."
            alert.alertStyle = .warning
            alert.addButton(withTitle: "Supprimer")
            alert.addButton(withTitle: "Annuler")

            guard let window = self.window else { return }
            alert.beginSheetModal(for: window) { [weak self] response in
                if response == .alertFirstButtonReturn {
                    OpenAIAPIEngine.removeConfig(id: configId)
                    if ASRModelCatalog.shared.selectedModelId == model.id {
                        ASRModelCatalog.shared.selectedModelId = "whisper:large-v3-turbo"
                    }
                    Log.info("Deleted API config: \(configId)")
                    let idx = max(0, self?.engineTableView?.selectedRow ?? 0)
                    self?.selectEngine(at: idx)
                    NotificationCenter.default.post(name: .modelDownloadCompleted, object: nil)
                }
            }
            return
        }

        let alert = NSAlert()
        alert.messageText = "Supprimer \(model.label) ?"
        alert.informativeText = "Le fichier du modèle sera supprimé. Vous pourrez le re-télécharger plus tard."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Supprimer")
        alert.addButton(withTitle: "Annuler")

        guard let window = self.window else { return }
        alert.beginSheetModal(for: window) { [weak self] response in
            if response == .alertFirstButtonReturn {
                if ModelDownloader.shared.deleteModel(model) {
                    Log.info("Deleted model: \(model.id)")
                    let idx = max(0, self?.engineTableView?.selectedRow ?? 0)
                    self?.selectEngine(at: idx)
                }
            }
        }
    }

    @objc func addAPIServerClicked(_ sender: NSButton) {
        showAPIConfigSheet(existing: nil)
    }

    @objc func editAPIServerClicked(_ sender: NSButton) {
        guard let model = modelFromRow(sender.tag) else { return }
        let configId = model.filename
        let configs = OpenAIAPIEngine.loadConfigs()
        guard let config = configs.first(where: { $0.id == configId }) else { return }
        showAPIConfigSheet(existing: config)
    }

    // MARK: - API Config Sheet

    private func showAPIConfigSheet(existing: APIServerConfig?) {
        guard let window = self.window else { return }

        let sheet = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 400, height: 280),
            styleMask: [.titled],
            backing: .buffered,
            defer: false
        )
        sheet.title = existing != nil ? "Modifier le serveur API" : "Ajouter un serveur API"

        let content = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 280))

        let labels = ["Nom :", "URL de base :", "Clé API :", "ID du modèle :", "Nom du modèle :"]
        let placeholders = ["Mon serveur", "http://localhost:8080/v1", "sk-... (optionnel)", "whisper-1", "Whisper 1"]
        let defaults = [
            existing?.name ?? "",
            existing?.baseURL ?? "http://localhost:8080/v1",
            existing?.apiKey ?? "",
            existing?.modelId ?? "whisper-1",
            existing?.modelLabel ?? "Whisper 1",
        ]

        var fields: [NSTextField] = []
        for i in 0..<labels.count {
            let y = CGFloat(240 - i * 46)

            let label = NSTextField(labelWithString: labels[i])
            label.font = NSFont.systemFont(ofSize: 12, weight: .medium)
            label.frame = NSRect(x: 20, y: y, width: 110, height: 18)
            label.alignment = .right
            content.addSubview(label)

            if labels[i] == "Clé API :" {
                let secure = NSSecureTextField(frame: NSRect(x: 136, y: y - 4, width: 244, height: 24))
                secure.placeholderString = placeholders[i]
                secure.stringValue = defaults[i]
                secure.font = NSFont.systemFont(ofSize: 12)
                content.addSubview(secure)
                fields.append(secure)
            } else {
                let field = NSTextField(frame: NSRect(x: 136, y: y - 4, width: 244, height: 24))
                field.placeholderString = placeholders[i]
                field.stringValue = defaults[i]
                field.font = NSFont.systemFont(ofSize: 12)
                content.addSubview(field)
                fields.append(field)
            }
        }

        let cancelBtn = NSButton(frame: NSRect(x: 210, y: 12, width: 80, height: 28))
        cancelBtn.title = "Annuler"
        cancelBtn.bezelStyle = .rounded
        cancelBtn.keyEquivalent = "\u{1b}"
        cancelBtn.target = self
        cancelBtn.action = #selector(dismissSheet(_:))
        content.addSubview(cancelBtn)

        let saveBtn = NSButton(frame: NSRect(x: 300, y: 12, width: 80, height: 28))
        saveBtn.title = existing != nil ? "Modifier" : "Ajouter"
        saveBtn.bezelStyle = .rounded
        saveBtn.keyEquivalent = "\r"
        content.addSubview(saveBtn)

        sheet.contentView = content

        let context = APISheetContext(fields: fields, existingId: existing?.id, sheet: sheet)
        objc_setAssociatedObject(saveBtn, &APISheetContext.key, context, .OBJC_ASSOCIATION_RETAIN)
        saveBtn.target = self
        saveBtn.action = #selector(saveAPIConfig(_:))

        window.beginSheet(sheet)
    }

    @objc private func dismissSheet(_ sender: NSButton) {
        guard let sheet = sender.window else { return }
        window?.endSheet(sheet)
    }

    @objc private func saveAPIConfig(_ sender: NSButton) {
        guard let context = objc_getAssociatedObject(sender, &APISheetContext.key) as? APISheetContext else { return }
        let fields = context.fields

        let name = fields[0].stringValue.trimmingCharacters(in: .whitespaces)
        let baseURL = fields[1].stringValue.trimmingCharacters(in: .whitespaces)
        let apiKey = fields[2].stringValue.trimmingCharacters(in: .whitespaces)
        let modelId = fields[3].stringValue.trimmingCharacters(in: .whitespaces)
        let modelLabel = fields[4].stringValue.trimmingCharacters(in: .whitespaces)

        guard !name.isEmpty, !baseURL.isEmpty, !modelId.isEmpty, !modelLabel.isEmpty else {
            NSSound(named: "Basso")?.play()
            return
        }

        if let existingId = context.existingId {
            var configs = OpenAIAPIEngine.loadConfigs()
            if let idx = configs.firstIndex(where: { $0.id == existingId }) {
                configs[idx].name = name
                configs[idx].baseURL = baseURL
                configs[idx].apiKey = apiKey
                configs[idx].modelId = modelId
                configs[idx].modelLabel = modelLabel
                OpenAIAPIEngine.saveConfigs(configs)
            }
        } else {
            let config = APIServerConfig(name: name, baseURL: baseURL, apiKey: apiKey, modelId: modelId, modelLabel: modelLabel)
            OpenAIAPIEngine.addConfig(config)
        }

        window?.endSheet(context.sheet)
        rebuildEngineList()
        engineTableView?.reloadData()
        let idx = max(0, engineTableView?.selectedRow ?? 0)
        selectEngine(at: idx)
        NotificationCenter.default.post(name: .modelDownloadCompleted, object: nil)
    }
}

// MARK: - Sheet context helper

private class APISheetContext {
    static var key: UInt8 = 0
    let fields: [NSTextField]
    let existingId: String?
    let sheet: NSWindow
    init(fields: [NSTextField], existingId: String?, sheet: NSWindow) {
        self.fields = fields
        self.existingId = existingId
        self.sheet = sheet
    }
}

// MARK: - NSSplitViewDelegate

extension ModelManagerWindowController: NSSplitViewDelegate {
    func splitView(_ splitView: NSSplitView, constrainMinCoordinate proposedMinimumPosition: CGFloat, ofSubviewAt dividerIndex: Int) -> CGFloat {
        160
    }
    func splitView(_ splitView: NSSplitView, constrainMaxCoordinate proposedMaximumPosition: CGFloat, ofSubviewAt dividerIndex: Int) -> CGFloat {
        220
    }
}

// MARK: - NSTableViewDataSource

extension ModelManagerWindowController: NSTableViewDataSource {
    func numberOfRows(in tableView: NSTableView) -> Int {
        if tableView === engineTableView { return displayEngines.count }
        return modelRows.count
    }
}

// MARK: - NSTableViewDelegate

extension ModelManagerWindowController: NSTableViewDelegate {
    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
        if tableView === engineTableView { return 44 }
        switch modelRows[row] {
        case .addAPIServer: return 40
        case .engineUnavailable: return 50
        case .model: return 56
        }
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        if tableView === engineTableView {
            let engine = displayEngines[row]
            let available = engineAvailable[engine.engineId] ?? false
            let cell = NSTableCellView(frame: NSRect(x: 0, y: 0, width: 180, height: 44))

            let title = NSTextField(labelWithString: engine.displayName)
            title.font = NSFont.systemFont(ofSize: 13, weight: .medium)
            title.textColor = available ? .labelColor : .tertiaryLabelColor
            title.frame = NSRect(x: 4, y: 22, width: 152, height: 18)
            cell.addSubview(title)

            let subtitle = NSTextField(labelWithString: toolSubtitle(for: engine))
            subtitle.font = NSFont.systemFont(ofSize: 11)
            subtitle.textColor = available ? .secondaryLabelColor : .tertiaryLabelColor
            subtitle.frame = NSRect(x: 4, y: 4, width: 152, height: 16)
            cell.addSubview(subtitle)

            // Status dot
            let dot = NSTextField(labelWithString: available ? "●" : "○")
            dot.font = NSFont.systemFont(ofSize: 8)
            dot.textColor = available ? .systemGreen : .tertiaryLabelColor
            dot.frame = NSRect(x: 162, y: 22, width: 14, height: 18)
            cell.addSubview(dot)

            return cell
        }

        switch modelRows[row] {
        case .engineUnavailable(let installHint):
            let container = NSView(frame: NSRect(x: 0, y: 0, width: 460, height: 50))
            let icon = NSTextField(labelWithString: "⚠")
            icon.font = NSFont.systemFont(ofSize: 14)
            icon.frame = NSRect(x: 12, y: 18, width: 20, height: 20)
            container.addSubview(icon)

            let msg = NSTextField(labelWithString: "CLI non installé — \(installHint)")
            msg.font = NSFont.systemFont(ofSize: 12)
            msg.textColor = .secondaryLabelColor
            msg.frame = NSRect(x: 34, y: 18, width: 400, height: 18)
            msg.isSelectable = true
            container.addSubview(msg)
            return container

        case .addAPIServer:
            let container = NSView(frame: NSRect(x: 0, y: 0, width: 460, height: 40))
            let button = NSButton(frame: NSRect(x: 12, y: 8, width: 160, height: 24))
            button.title = "Ajouter un serveur…"
            button.bezelStyle = .rounded
            button.controlSize = .small
            button.target = self
            button.action = #selector(addAPIServerClicked(_:))
            container.addSubview(button)
            return container

        case .model(let model):
            let isSelected = model.id == ASRModelCatalog.shared.selectedModelId
            let isDownloading = model.id == state.downloadingModelId
            let available = engineAvailable[model.engineId] ?? false
            let cell = ModelCellView(frame: NSRect(x: 0, y: 0, width: 460, height: 56))
            cell.configure(
                model: model,
                isSelected: isSelected,
                isDownloading: isDownloading,
                progress: isDownloading ? state.downloadProgress : 0,
                anyDownloading: state.downloadingModelId != nil,
                engineAvailable: available,
                row: row,
                target: self
            )
            return cell
        }
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        guard let tv = notification.object as? NSTableView, tv === engineTableView else { return }
        let row = tv.selectedRow
        guard row >= 0 else { return }
        selectEngine(at: row)
    }
}

