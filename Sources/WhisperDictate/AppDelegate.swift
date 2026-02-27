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
    private var postProcessItem: NSMenuItem!

    // Queue system
    private var pendingTranscribingTransition: DispatchWorkItem?

    // Double-tap cancel
    private var lastKeyDownTime: Date?
    private var lastShortTapTime: Date?

    // Post-processing
    private var postProcessingEnabled: Bool {
        get { UserDefaults.standard.bool(forKey: "postProcessingEnabled") }
        set { UserDefaults.standard.set(newValue, forKey: "postProcessingEnabled") }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        UserDefaults.standard.register(defaults: ["postProcessingEnabled": true])
        cleanupOrphanAudioFiles()
        AudioDeviceManager.applySavedDevice()
        AudioDeviceManager.startDeviceChangeListener()
        ModelManagerWindowController.shared.pill = pill
        setupMenuBar()

        NotificationCenter.default.addObserver(self, selector: #selector(downloadDidComplete), name: .modelDownloadCompleted, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(audioDevicesChanged), name: .audioDevicesChanged, object: nil)

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

        postProcessItem = NSMenuItem(title: "Post-traitement", action: #selector(togglePostProcessing), keyEquivalent: "")
        postProcessItem.target = self
        menu.addItem(postProcessItem)

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

    @objc private func audioDevicesChanged() {
        guard !AudioDeviceManager.isSavedDeviceConnected() else { return }
        Log.info("Saved mic disconnected")
        if state.isRecording {
            stopRecordingAndEnqueue()
            NSSound(named: "Basso")?.play()
            NotificationService.show(
                title: "Micro déconnecté",
                body: "L'enregistrement a été arrêté. Le micro par défaut sera utilisé."
            )
        }
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
        state.transcriptionCancelled = false
        lastKeyDownTime = Date()

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

        let audioURL = recorder.stopRecording()

        // Detect short tap (< 300ms press duration)
        let isShortTap: Bool
        if let downTime = lastKeyDownTime {
            isShortTap = Date().timeIntervalSince(downTime) < 0.3
        } else {
            isShortTap = false
        }
        lastKeyDownTime = nil

        if isShortTap {
            // Discard audio from short tap
            if let url = audioURL { try? FileManager.default.removeItem(at: url) }

            if let lastTap = lastShortTapTime, Date().timeIntervalSince(lastTap) < 0.5 {
                // Double-tap → cancel transcription
                lastShortTapTime = nil
                cancelTranscription()
                return
            }
            lastShortTapTime = Date()

            // Single short tap — maintain current UI state
            if !state.isTranscribing && state.transcriptionQueue.isEmpty && !state.isDownloading {
                pill.dismiss()
            } else if state.isTranscribing {
                scheduleTranscribingTransition()
            }
            DispatchQueue.main.async { self.setMenuBarIcon("mic.fill") }
            return
        }

        lastShortTapTime = nil

        guard let audioURL = audioURL else {
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
        let count = state.enqueue(audioURL)
        pill.queueCount = count
        Log.info("Enqueued transcription (\(count) in queue)")

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
            if let audioURL = state.dequeue() {
                try? FileManager.default.removeItem(at: audioURL)
            }
            state.transcriptionQueue = []
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
        guard let audioURL = state.dequeue() else {
            state.isTranscribing = false
            return
        }
        pill.queueCount = state.queueCount

        if !state.isRecording {
            if debounceTransition {
                scheduleTranscribingTransition()
            } else {
                pill.showTranscribing()
                setMenuBarIcon("text.bubble")
            }
        }

        Task { @MainActor [weak self] in
            guard let self = self else { return }

            var hadError = false

            do {
                let text = try await self.transcriber.transcribe(audioURL: audioURL)
                try? FileManager.default.removeItem(at: audioURL)

                guard !self.state.transcriptionCancelled else {
                    Log.info("Transcription result discarded (cancelled)")
                    self.state.isTranscribing = false
                    self.processNextInQueue()
                    return
                }

                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty {
                    let processed = self.postProcessingEnabled
                        ? TextPostProcessor.process(trimmed, language: ASRModelCatalog.shared.selectedLanguage)
                        : trimmed
                    self.pasteService.paste(text: processed)
                    NSSound(named: "Glass")?.play()
                } else {
                    NSSound(named: "Basso")?.play()
                }
            } catch {
                hadError = true
                try? FileManager.default.removeItem(at: audioURL)
                NSSound(named: "Basso")?.play()
                Log.error("Transcription error: \(error)")

                if let te = error as? TranscriberError {
                    let msg = te.userMessage
                    NotificationService.show(title: msg.title, body: msg.body)
                } else {
                    NotificationService.show(title: "Erreur de transcription", body: error.localizedDescription)
                }
            }

            self.state.isTranscribing = false

            if hadError && self.state.transcriptionQueue.isEmpty && !self.state.isRecording {
                self.pill.showError()
                DispatchQueue.main.async { self.setMenuBarIcon("mic.fill") }
            } else {
                self.processNextInQueue()
            }
        }
    }

    private func cancelTranscription() {
        // Clear the queue
        while let url = state.dequeue() {
            try? FileManager.default.removeItem(at: url)
        }
        pill.queueCount = 0
        state.transcriptionCancelled = true

        Log.info("Transcription cancelled (double-tap)")
        NSSound(named: "Funk")?.play()
        pill.showError()
        DispatchQueue.main.async { self.setMenuBarIcon("mic.fill") }
    }

    @objc private func togglePostProcessing() {
        postProcessingEnabled = !postProcessingEnabled
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

    private func cleanupOrphanAudioFiles() {
        let tmpDir = FileManager.default.temporaryDirectory
        guard let files = try? FileManager.default.contentsOfDirectory(
            at: tmpDir, includingPropertiesForKeys: [.contentModificationDateKey]
        ) else { return }
        let cutoff = Date().addingTimeInterval(-300)
        for file in files where file.lastPathComponent.hasPrefix("whisper_dictate_") && file.pathExtension == "wav" {
            if let vals = try? file.resourceValues(forKeys: [.contentModificationDateKey]),
               let modified = vals.contentModificationDate, modified < cutoff {
                try? FileManager.default.removeItem(at: file)
                Log.info("Cleaned orphan audio: \(file.lastPathComponent)")
            }
        }
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
        postProcessItem.state = postProcessingEnabled ? .on : .off
    }
}
