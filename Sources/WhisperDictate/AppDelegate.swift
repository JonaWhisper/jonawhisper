import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private var keyMonitor: KeyMonitor!
    private let recorder = AudioRecorder()
    private let transcriber = Transcriber()
    private let pasteService = PasteService()
    private let pill = FloatingPill()
    private var state: AppState { AppState.shared }
    private var micItem: NSMenuItem!
    private var micSubmenu: NSMenu!
    private var langItem: NSMenuItem!
    private var langSubmenu: NSMenu!
    private var modelItem: NSMenuItem!
    private var modelSubmenu: NSMenu!

    // Queue system
    private var pendingTranscribingTransition: DispatchWorkItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        AudioDeviceManager.applySavedDevice()
        ModelManagerWindowController.shared.pill = pill
        setupMenuBar()

        NotificationCenter.default.addObserver(self, selector: #selector(downloadDidComplete), name: .modelDownloadCompleted, object: nil)

        PermissionChecker.check { [weak self] report in
            report.log()
            if report.inputMonitoring == .granted {
                self?.setupKeyMonitor()
                Log.info("Ready")
            } else {
                self?.setMenuBarIcon("mic.slash")
                Log.info("Waiting for Input Monitoring permission...")
            }
        }

        // Resume pending download from previous session
        resumePendingDownload()
    }

    func applicationWillTerminate(_ notification: Notification) {
        ModelDownloader.shared.saveResumeDataAndCancel()
        // Give a moment for the resume data to be written
        Thread.sleep(forTimeInterval: 0.5)
    }

    private func setupMenuBar() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = statusItem.button {
            button.image = NSImage(systemSymbolName: "mic.fill", accessibilityDescription: "WhisperDictate")
        }

        let menu = NSMenu()
        menu.delegate = self
        menu.addItem(NSMenuItem(title: "WhisperDictate", action: nil, keyEquivalent: ""))
        menu.addItem(NSMenuItem.separator())

        micItem = NSMenuItem(title: "Microphone", action: nil, keyEquivalent: "")
        micSubmenu = NSMenu()
        micItem.submenu = micSubmenu
        menu.addItem(micItem)

        langItem = NSMenuItem(title: "Langue", action: nil, keyEquivalent: "")
        langSubmenu = NSMenu()
        langItem.submenu = langSubmenu
        menu.addItem(langItem)

        modelItem = NSMenuItem(title: "Modèle", action: nil, keyEquivalent: "")
        modelSubmenu = NSMenu()
        modelItem.submenu = modelSubmenu
        menu.addItem(modelItem)

        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Quit", action: #selector(quit), keyEquivalent: "q"))
        statusItem.menu = menu
    }

    private func refreshMicMenu() {
        micSubmenu.removeAllItems()

        let devices = AudioDeviceManager.listInputDevices()
        let currentDefault = AudioDeviceManager.getDefaultInputDevice()
        let savedUID = AudioDeviceManager.getSavedDeviceUID()

        var activeName: String?

        for device in devices {
            let item = NSMenuItem(
                title: device.name,
                action: #selector(selectMic(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = device
            item.image = NSImage(systemSymbolName: device.transportType.symbolName, accessibilityDescription: nil)

            let isActive: Bool
            if let savedUID = savedUID {
                isActive = device.uid == savedUID
            } else {
                isActive = device.id == currentDefault
            }
            item.state = isActive ? .on : .off
            if isActive { activeName = device.name }

            micSubmenu.addItem(item)
        }

        if devices.isEmpty {
            let noDevices = NSMenuItem(title: "No input devices found", action: nil, keyEquivalent: "")
            noDevices.isEnabled = false
            micSubmenu.addItem(noDevices)
        }

        micItem.title = activeName ?? "Microphone"
        if let activeDevice = devices.first(where: { d in
            if let savedUID = savedUID { return d.uid == savedUID }
            return d.id == currentDefault
        }) {
            micItem.image = NSImage(systemSymbolName: activeDevice.transportType.symbolName, accessibilityDescription: nil)
        }
    }

    @objc private func selectMic(_ sender: NSMenuItem) {
        guard let device = sender.representedObject as? AudioDevice else { return }
        AudioDeviceManager.setDefaultInputDevice(device.id)
        AudioDeviceManager.saveSelectedDevice(uid: device.uid)
        Log.info("Selected mic: \(device.name)")

        for item in micSubmenu.items {
            item.state = (item.representedObject as? AudioDevice) == device ? .on : .off
        }
    }

    private func refreshLangMenu() {
        langSubmenu.removeAllItems()

        let catalog = ASRModelCatalog.shared
        let current = catalog.selectedLanguage
        var activeLabel = "Auto"
        for lang in catalog.supportedLanguages {
            let item = NSMenuItem(
                title: lang.label,
                action: #selector(selectLang(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = lang.code
            let isActive = lang.code == current
            item.state = isActive ? .on : .off
            if isActive { activeLabel = lang.label }
            langSubmenu.addItem(item)
        }

        langItem.title = "Langue: \(activeLabel)"
    }

    @objc private func selectLang(_ sender: NSMenuItem) {
        guard let code = sender.representedObject as? String else { return }
        ASRModelCatalog.shared.selectedLanguage = code
        Log.info("Selected language: \(code)")

        for item in langSubmenu.items {
            item.state = (item.representedObject as? String) == code ? .on : .off
        }
    }

    private func refreshModelMenu() {
        modelSubmenu.removeAllItems()

        let catalog = ASRModelCatalog.shared
        let current = catalog.selectedModelId
        let downloaded = catalog.downloadedModels

        for model in downloaded {
            let item = NSMenuItem(
                title: model.label,
                action: #selector(selectModel(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = model.id
            item.state = model.id == current ? .on : .off
            modelSubmenu.addItem(item)
        }

        if downloaded.isEmpty {
            let none = NSMenuItem(title: "Aucun modèle installé", action: nil, keyEquivalent: "")
            none.isEnabled = false
            modelSubmenu.addItem(none)
        }

        modelSubmenu.addItem(NSMenuItem.separator())

        let manage = NSMenuItem(title: "Gérer les modèles…", action: #selector(openModelManager), keyEquivalent: "")
        manage.target = self
        modelSubmenu.addItem(manage)

        if let selected = downloaded.first(where: { $0.id == current }) {
            modelItem.title = "Modèle: \(selected.label)"
        } else {
            modelItem.title = "⚠ Modèle indisponible"
        }
    }

    @objc private func selectModel(_ sender: NSMenuItem) {
        guard let modelId = sender.representedObject as? String else { return }
        ASRModelCatalog.shared.selectedModelId = modelId
        Log.info("Selected model: \(modelId)")
    }

    @objc private func openModelManager() {
        ModelManagerWindowController.showWindow()
    }

    @objc private func downloadDidComplete() {
        if !state.isRecording && !state.isTranscribing && state.transcriptionQueue.isEmpty {
            pill.dismiss()
        }
    }

    private func setupKeyMonitor() {
        keyMonitor = KeyMonitor(
            onKeyDown: { [weak self] in
                self?.startRecording()
            },
            onKeyUp: { [weak self] in
                self?.stopRecordingAndEnqueue()
            }
        )
        keyMonitor.start()
    }

    // MARK: - Recording

    private func startRecording() {
        guard !state.isRecording else { return }
        state.isRecording = true

        // Cancel pending transition to transcribing dots
        pendingTranscribingTransition?.cancel()
        pendingTranscribingTransition = nil

        AudioDeviceManager.applySavedDevice()

        DispatchQueue.main.async {
            self.setMenuBarIcon("mic.badge.plus")
        }

        NSSound(named: "Tink")?.play()
        recorder.startRecording()

        pill.show { [weak self] in
            let data = self?.recorder.getSpectrum()
            if let data = data, !data.isEmpty { return data }
            return Array(repeating: Float(0), count: 12)
        }
    }

    private func stopRecordingAndEnqueue() {
        guard state.isRecording else { return }
        state.isRecording = false

        guard let audioURL = recorder.stopRecording() else {
            Log.info("No audio recorded")
            if !state.isTranscribing && state.transcriptionQueue.isEmpty && !state.isDownloading {
                pill.dismiss()
            } else if state.isTranscribing {
                scheduleTranscribingTransition()
            }
            DispatchQueue.main.async {
                self.setMenuBarIcon("mic.fill")
            }
            return
        }

        NSSound(named: "Pop")?.play()

        // Add to queue
        state.transcriptionQueue.append(audioURL)
        pill.queueCount = state.transcriptionQueue.count
        Log.info("Enqueued transcription (\(state.transcriptionQueue.count) in queue)")

        // Process queue if not already processing
        processNextInQueue(debounceTransition: true)

        // If a transcription was already running (processNextInQueue returned early),
        // still switch to transcribing dots since the user stopped recording
        if state.isTranscribing {
            scheduleTranscribingTransition()
        }
    }

    // MARK: - Queue

    private func scheduleTranscribingTransition() {
        pendingTranscribingTransition?.cancel()
        let item = DispatchWorkItem { [weak self] in
            guard let self = self, !self.state.isRecording, self.state.isTranscribing else { return }
            self.pill.showTranscribing()
            self.setMenuBarIcon("text.bubble")
        }
        self.pendingTranscribingTransition = item
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: item)
    }

    private func processNextInQueue(debounceTransition: Bool = false) {
        guard !state.isTranscribing else { return }
        guard !state.transcriptionQueue.isEmpty else {
            // All done
            if !state.isRecording {
                if !state.isDownloading {
                    pill.dismiss()
                }
                DispatchQueue.main.async {
                    self.setMenuBarIcon("mic.fill")
                }
            }
            return
        }

        // Check if the selected model is available
        let model = ASRModelCatalog.shared.selectedModel
        guard model.isDownloaded else {
            // No usable model — discard queue and notify
            let audioURL = state.transcriptionQueue.removeFirst()
            try? FileManager.default.removeItem(at: audioURL)
            pill.queueCount = state.transcriptionQueue.count
            state.transcriptionQueue.removeAll()
            pill.queueCount = 0
            NotificationService.show(title: "Modèle indisponible", body: "Le modèle \(model.label) n'est pas téléchargé. Ouvrez Modèles… pour en choisir un.")
            NSSound(named: "Basso")?.play()
            if !state.isRecording && !state.isDownloading {
                pill.dismiss()
            }
            DispatchQueue.main.async {
                self.setMenuBarIcon("mic.fill")
            }
            return
        }

        state.isTranscribing = true
        let audioURL = state.transcriptionQueue.removeFirst()
        pill.queueCount = state.transcriptionQueue.count

        if !state.isRecording {
            if debounceTransition {
                scheduleTranscribingTransition()
            } else {
                pill.showTranscribing()
                setMenuBarIcon("text.bubble")
            }
        }

        transcriber.transcribe(audioURL: audioURL) { [weak self] result in
            guard let self = self else { return }

            try? FileManager.default.removeItem(at: audioURL)

            switch result {
            case .success(let text):
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty {
                    self.pasteService.paste(text: trimmed)
                    NSSound(named: "Glass")?.play()
                } else {
                    NSSound(named: "Basso")?.play()
                }

            case .failure(let error):
                NSSound(named: "Basso")?.play()
                Log.error("Transcription error: \(error)")
            }

            self.state.isTranscribing = false
            self.processNextInQueue()
        }
    }

    private func setMenuBarIcon(_ symbolName: String) {
        if let button = statusItem.button {
            button.image = NSImage(systemSymbolName: symbolName, accessibilityDescription: "WhisperDictate")
        }
    }

    private func resumePendingDownload() {
        guard let modelId = ModelDownloader.shared.pendingDownloadModelId(),
              let model = ASRModelCatalog.shared.model(byId: modelId) else { return }

        Log.info("Resuming pending download: \(model.id)")
        ModelManagerWindowController.shared.startDownload(model)
    }

    @objc private func quit() {
        keyMonitor?.stop()
        NSApplication.shared.terminate(nil)
    }
}

// MARK: - NSMenuDelegate
extension AppDelegate: NSMenuDelegate {
    func menuWillOpen(_ menu: NSMenu) {
        refreshMicMenu()
        refreshLangMenu()
        refreshModelMenu()
    }
}
