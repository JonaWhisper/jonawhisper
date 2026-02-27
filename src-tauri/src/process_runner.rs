use crate::engines::EngineError;
use std::process::Command;

pub struct ProcessResult {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run(executable: &str, arguments: &[String]) -> Result<ProcessResult, EngineError> {
    let output = Command::new(executable)
        .args(arguments)
        .output()
        .map_err(|e| EngineError::LaunchFailed(format!("{}: {}", executable, e)))?;

    let result = ProcessResult {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    };

    if !output.status.success() {
        log::error!("{} failed (status {}): {}", executable, result.status, result.stderr);
        return Err(EngineError::ProcessFailed {
            code: result.status,
            stderr: result.stderr,
        });
    }

    Ok(result)
}
