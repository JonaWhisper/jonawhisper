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
        } catch {
            throw TranscriberError.launchFailed(error)
        }

        // Read pipes concurrently to avoid deadlock when output exceeds buffer
        var stdoutData = Data()
        var stderrData = Data()
        let group = DispatchGroup()
        group.enter()
        DispatchQueue.global().async {
            stdoutData = stdoutPipe.fileHandleForReading.readDataToEndOfFile()
            group.leave()
        }
        group.enter()
        DispatchQueue.global().async {
            stderrData = stderrPipe.fileHandleForReading.readDataToEndOfFile()
            group.leave()
        }
        process.waitUntilExit()
        group.wait()

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
