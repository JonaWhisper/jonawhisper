import Foundation

struct ProcessResult {
    let status: Int32
    let stdout: String
    let stderr: String
}

enum ProcessRunner {
    /// Run a process synchronously and return its output.
    static func run(executable: String, arguments: [String]) throws -> ProcessResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments

        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            throw TranscriberError.launchFailed(error)
        }

        let stdoutData = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
        let stderrData = stderrPipe.fileHandleForReading.readDataToEndOfFile()

        let result = ProcessResult(
            status: process.terminationStatus,
            stdout: String(data: stdoutData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? "",
            stderr: String(data: stderrData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        )

        if result.status != 0 {
            Log.error("\(executable) failed (status \(result.status)): \(result.stderr)")
            throw TranscriberError.processFailed(result.status, result.stderr)
        }

        return result
    }
}
