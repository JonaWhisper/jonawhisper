import Cocoa
import AVFoundation
import UserNotifications

class SetupWindowController: NSWindowController {

    var onComplete: (() -> Void)?

    private var statusIcons: [NSTextField] = []
    private var actionButtons: [NSButton] = []
    private var continueButton: NSButton!
    private var pollTimer: Timer?

    private static let steps: [(name: String, desc: String, settingsURL: String)] = [
        ("Microphone",
         "Enregistrer votre voix pour la transcription",
         "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"),
        ("Accessibilité",
         "Coller le texte transcrit dans l'app active",
         "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"),
        ("Surveillance de l'entrée",
         "Détecter la touche Commande droite",
         "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"),
        ("Notifications",
         "Afficher les messages d'erreur",
         "x-apple.systempreferences:com.apple.Notifications-Settings.extension"),
    ]

    init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 460, height: 380),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "WhisperDictate"
        window.center()
        window.isReleasedWhenClosed = false
        super.init(window: window)
        setupUI()
        refreshStatuses()
        startPolling()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        pollTimer?.invalidate()
    }

    // MARK: - UI

    private func setupUI() {
        guard let contentView = window?.contentView else { return }

        let title = NSTextField(labelWithString: "Configuration des permissions")
        title.font = NSFont.systemFont(ofSize: 16, weight: .semibold)
        title.alignment = .center
        title.frame = NSRect(x: 0, y: 332, width: 460, height: 24)
        contentView.addSubview(title)

        let subtitle = NSTextField(labelWithString: "Accordez les permissions nécessaires au fonctionnement.")
        subtitle.font = NSFont.systemFont(ofSize: 12)
        subtitle.textColor = .secondaryLabelColor
        subtitle.alignment = .center
        subtitle.frame = NSRect(x: 20, y: 308, width: 420, height: 18)
        contentView.addSubview(subtitle)

        for (i, step) in Self.steps.enumerated() {
            let y = CGFloat(244 - i * 62)

            // Step number
            let num = NSTextField(labelWithString: "\(i + 1).")
            num.font = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .bold)
            num.textColor = .tertiaryLabelColor
            num.frame = NSRect(x: 20, y: y + 12, width: 20, height: 18)
            num.alignment = .right
            contentView.addSubview(num)

            // Status icon
            let icon = NSTextField(labelWithString: "○")
            icon.font = NSFont.systemFont(ofSize: 14)
            icon.textColor = .tertiaryLabelColor
            icon.frame = NSRect(x: 46, y: y + 12, width: 18, height: 18)
            contentView.addSubview(icon)
            statusIcons.append(icon)

            // Name
            let name = NSTextField(labelWithString: step.name)
            name.font = NSFont.systemFont(ofSize: 13, weight: .medium)
            name.frame = NSRect(x: 68, y: y + 20, width: 230, height: 18)
            contentView.addSubview(name)

            // Description
            let desc = NSTextField(labelWithString: step.desc)
            desc.font = NSFont.systemFont(ofSize: 11)
            desc.textColor = .secondaryLabelColor
            desc.frame = NSRect(x: 68, y: y + 2, width: 250, height: 16)
            contentView.addSubview(desc)

            // Action button
            let btn = NSButton(frame: NSRect(x: 340, y: y + 10, width: 100, height: 24))
            btn.title = "Accorder"
            btn.bezelStyle = .rounded
            btn.controlSize = .small
            btn.target = self
            btn.action = #selector(grantPermission(_:))
            btn.tag = i
            contentView.addSubview(btn)
            actionButtons.append(btn)
        }

        // Separator
        let sep = NSBox(frame: NSRect(x: 20, y: 50, width: 420, height: 1))
        sep.boxType = .separator
        contentView.addSubview(sep)

        // Continue button
        continueButton = NSButton(frame: NSRect(x: 340, y: 14, width: 100, height: 28))
        continueButton.title = "Continuer"
        continueButton.bezelStyle = .rounded
        continueButton.keyEquivalent = "\r"
        continueButton.target = self
        continueButton.action = #selector(continueClicked)
        continueButton.isEnabled = false
        contentView.addSubview(continueButton)

        // Optional hint
        let hint = NSTextField(labelWithString: "Notifications est optionnel")
        hint.font = NSFont.systemFont(ofSize: 10)
        hint.textColor = .tertiaryLabelColor
        hint.frame = NSRect(x: 20, y: 18, width: 200, height: 14)
        contentView.addSubview(hint)
    }

    // MARK: - Actions

    @objc private func grantPermission(_ sender: NSButton) {
        switch sender.tag {
        case 0: // Microphone
            if AVCaptureDevice.authorizationStatus(for: .audio) == .notDetermined {
                AVCaptureDevice.requestAccess(for: .audio) { _ in
                    DispatchQueue.main.async { self.refreshStatuses() }
                }
            } else {
                openSettings(Self.steps[0].settingsURL)
            }

        case 1: // Accessibility
            if !AXIsProcessTrusted() {
                let opts = [kAXTrustedCheckOptionPrompt.takeUnretainedValue(): true] as CFDictionary
                _ = AXIsProcessTrustedWithOptions(opts)
            }

        case 2: // Input Monitoring
            openSettings(Self.steps[2].settingsURL)

        case 3: // Notifications
            UNUserNotificationCenter.current().getNotificationSettings { settings in
                DispatchQueue.main.async {
                    if settings.authorizationStatus == .notDetermined {
                        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in
                            DispatchQueue.main.async { self.refreshStatuses() }
                        }
                    } else {
                        self.openSettings(Self.steps[3].settingsURL)
                    }
                }
            }

        default: break
        }
    }

    private func openSettings(_ urlString: String) {
        if let url = URL(string: urlString) {
            NSWorkspace.shared.open(url)
        }
    }

    @objc private func continueClicked() {
        pollTimer?.invalidate()
        pollTimer = nil
        window?.close()
        onComplete?()
    }

    // MARK: - Status polling

    private func startPolling() {
        pollTimer = Timer.scheduledTimer(withTimeInterval: 1.5, repeats: true) { [weak self] _ in
            self?.refreshStatuses()
        }
    }

    private func refreshStatuses() {
        let micGranted = AVCaptureDevice.authorizationStatus(for: .audio) == .authorized
        let accGranted = AXIsProcessTrusted()
        let inputGranted = PermissionChecker.checkInputMonitoring()

        UNUserNotificationCenter.current().getNotificationSettings { [weak self] settings in
            let notifGranted = settings.authorizationStatus == .authorized
                || settings.authorizationStatus == .provisional

            DispatchQueue.main.async {
                guard let self = self else { return }
                let statuses = [micGranted, accGranted, inputGranted, notifGranted]

                for (i, granted) in statuses.enumerated() {
                    self.statusIcons[i].stringValue = granted ? "●" : "○"
                    self.statusIcons[i].textColor = granted ? .systemGreen : .tertiaryLabelColor

                    if granted {
                        self.actionButtons[i].title = "Accordé ✓"
                        self.actionButtons[i].isEnabled = false
                    } else {
                        self.actionButtons[i].title = "Accorder"
                        self.actionButtons[i].isEnabled = true
                    }
                }

                // Continue requires mic + accessibility + input monitoring
                self.continueButton.isEnabled = micGranted && accGranted && inputGranted
            }
        }
    }
}
