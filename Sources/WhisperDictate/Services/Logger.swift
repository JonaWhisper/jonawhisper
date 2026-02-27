import Foundation
import os

enum Log {
    private static let logger = os.Logger(subsystem: "com.local.WhisperDictate", category: "app")
    private static let maxLogSize: UInt64 = 2 * 1024 * 1024 // 2 Mo
    private static let logFile: URL = {
        let url = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".local/share/whisper-dictate.log")
        try? FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        return url
    }()

    static func info(_ message: String) {
        logger.info("\(message)")
        appendToFile("INFO  \(message)")
    }

    static func error(_ message: String) {
        logger.error("\(message)")
        appendToFile("ERROR \(message)")
    }

    private static func appendToFile(_ message: String) {
        let timestamp = ISO8601DateFormatter().string(from: Date())
        let line = "\(timestamp) \(message)\n"
        guard let data = line.data(using: .utf8) else { return }

        if FileManager.default.fileExists(atPath: logFile.path) {
            rotateIfNeeded()
            if let handle = try? FileHandle(forWritingTo: logFile) {
                handle.seekToEndOfFile()
                handle.write(data)
                handle.closeFile()
            }
        } else {
            try? data.write(to: logFile)
        }
    }

    private static func rotateIfNeeded() {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: logFile.path),
              let size = attrs[.size] as? UInt64,
              size > maxLogSize else { return }
        let oldPath = logFile.path + ".old"
        try? FileManager.default.removeItem(atPath: oldPath)
        try? FileManager.default.moveItem(atPath: logFile.path, toPath: oldPath)
    }
}
