import Foundation
import os

enum Log {
    private static let logger = os.Logger(subsystem: "com.local.WhisperDictate", category: "app")
    private static let logFile: URL = {
        let url = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".local/share/whisper-dictate.log")
        // Ensure parent dir exists
        try? FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        return url
    }()

    static func info(_ message: String) {
        logger.info("\(message)")
        NSLog("WhisperDictate: %@", message)
        appendToFile("INFO  \(message)")
    }

    static func error(_ message: String) {
        logger.error("\(message)")
        NSLog("WhisperDictate ERROR: %@", message)
        appendToFile("ERROR \(message)")
    }

    private static func appendToFile(_ message: String) {
        let timestamp = ISO8601DateFormatter().string(from: Date())
        let line = "\(timestamp) \(message)\n"
        if let data = line.data(using: .utf8) {
            if FileManager.default.fileExists(atPath: logFile.path) {
                if let handle = try? FileHandle(forWritingTo: logFile) {
                    handle.seekToEndOfFile()
                    handle.write(data)
                    handle.closeFile()
                }
            } else {
                try? data.write(to: logFile)
            }
        }
    }
}
