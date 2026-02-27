import Cocoa

class ModelCellView: NSView {
    private var progressBar: NSProgressIndicator?
    private var progressLabel: NSTextField?

    func configure(model: ASRModel, isSelected: Bool, isDownloading: Bool, progress: Double, anyDownloading: Bool, engineAvailable: Bool = true, row: Int, target: ModelManagerWindowController) {
        let radio = NSButton(frame: NSRect(x: 10, y: 14, width: 28, height: 28))
        radio.isBordered = false
        radio.image = NSImage(systemSymbolName: isSelected ? "circle.inset.filled" : "circle", accessibilityDescription: "Sélectionner")
        radio.contentTintColor = isSelected ? .controlAccentColor : .tertiaryLabelColor
        radio.target = target
        radio.action = #selector(ModelManagerWindowController.radioClicked(_:))
        radio.tag = row
        radio.isEnabled = model.isDownloaded
        addSubview(radio)

        let nameLabel = NSTextField(labelWithString: model.label)
        nameLabel.font = NSFont.systemFont(ofSize: 13, weight: .medium)
        nameLabel.frame = NSRect(x: 44, y: 30, width: 200, height: 18)
        addSubview(nameLabel)

        let sizeLabel = NSTextField(labelWithString: model.size)
        sizeLabel.font = NSFont.systemFont(ofSize: 11)
        sizeLabel.textColor = .secondaryLabelColor
        sizeLabel.frame = NSRect(x: 44, y: 10, width: 200, height: 16)
        addSubview(sizeLabel)

        if model.isRemoteAPI {
            let apiLabel = NSTextField(labelWithString: "API")
            apiLabel.font = NSFont.systemFont(ofSize: 11)
            apiLabel.textColor = .systemBlue
            apiLabel.frame = NSRect(x: 280, y: 20, width: 60, height: 16)
            addSubview(apiLabel)

            let editButton = NSButton(frame: NSRect(x: 400, y: 16, width: 24, height: 24))
            editButton.image = NSImage(systemSymbolName: "pencil", accessibilityDescription: "Modifier")
            editButton.bezelStyle = .recessed
            editButton.isBordered = false
            editButton.target = target
            editButton.action = #selector(ModelManagerWindowController.editAPIServerClicked(_:))
            editButton.tag = row
            addSubview(editButton)

            let deleteButton = NSButton(frame: NSRect(x: 436, y: 16, width: 24, height: 24))
            deleteButton.image = NSImage(systemSymbolName: "trash", accessibilityDescription: "Supprimer")
            deleteButton.bezelStyle = .recessed
            deleteButton.isBordered = false
            deleteButton.target = target
            deleteButton.action = #selector(ModelManagerWindowController.deleteClicked(_:))
            deleteButton.tag = row
            addSubview(deleteButton)
        } else if isDownloading {
            let bar = NSProgressIndicator(frame: NSRect(x: 280, y: 26, width: 100, height: 14))
            bar.style = .bar
            bar.isIndeterminate = false
            bar.minValue = 0
            bar.maxValue = 1
            bar.doubleValue = progress
            addSubview(bar)
            self.progressBar = bar

            let pctLabel = NSTextField(labelWithString: "\(Int(progress * 100))%")
            pctLabel.font = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .regular)
            pctLabel.textColor = .secondaryLabelColor
            pctLabel.frame = NSRect(x: 385, y: 24, width: 40, height: 16)
            addSubview(pctLabel)
            self.progressLabel = pctLabel
        } else if model.isDownloaded {
            let check = NSTextField(labelWithString: "✓ Téléchargé")
            check.font = NSFont.systemFont(ofSize: 11)
            check.textColor = .systemGreen
            check.frame = NSRect(x: 280, y: 20, width: 100, height: 16)
            addSubview(check)
        } else {
            let dlButton = NSButton(frame: NSRect(x: 280, y: 16, width: 95, height: 24))
            dlButton.title = "Télécharger"
            dlButton.bezelStyle = .rounded
            dlButton.controlSize = .small
            dlButton.target = target
            dlButton.action = #selector(ModelManagerWindowController.downloadClicked(_:))
            dlButton.tag = row
            dlButton.isEnabled = !anyDownloading && engineAvailable
            addSubview(dlButton)
        }

        if !model.isRemoteAPI {
            let deleteButton = NSButton(frame: NSRect(x: 436, y: 16, width: 24, height: 24))
            deleteButton.image = NSImage(systemSymbolName: "trash", accessibilityDescription: "Supprimer")
            deleteButton.bezelStyle = .recessed
            deleteButton.isBordered = false
            deleteButton.target = target
            deleteButton.action = #selector(ModelManagerWindowController.deleteClicked(_:))
            deleteButton.tag = row
            deleteButton.isEnabled = model.isDownloaded && !isDownloading
            addSubview(deleteButton)
        }
    }

    func updateProgress(_ progress: Double) {
        progressBar?.doubleValue = progress
        progressLabel?.stringValue = "\(Int(progress * 100))%"
    }
}
