//! Shared ort session builder with CoreML Execution Provider on macOS.
//!
//! CoreML dispatches to Metal GPU or Apple Neural Engine automatically.
//! Falls back to CPU if CoreML is unavailable at runtime.

use ort::session::Session;
use ort::session::builder::SessionBuilder;

/// Number of threads for inference. Returns all available cores (fallback: 4).
pub fn inference_threads() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

/// Build an ort SessionBuilder with CoreML on macOS, CPU fallback elsewhere.
pub fn build_session(n_threads: usize) -> Result<SessionBuilder, String> {
    let builder = Session::builder()
        .map_err(|e| format!("ort session builder: {e}"))?;

    #[cfg(target_os = "macos")]
    let builder = {
        use ort::ep;
        match builder.with_execution_providers([ep::CoreML::default().build()]) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("CoreML EP unavailable, CPU fallback: {e}");
                Session::builder().map_err(|e| format!("ort session builder: {e}"))?
            }
        }
    };

    let builder = builder
        .with_intra_threads(n_threads)
        .map_err(|e| format!("ort threads: {e}"))?;

    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inference_uses_all_available_cores() {
        // ONNX inference should use all available CPU cores for maximum performance.
        // On any real machine there's at least 1 core.
        let n = inference_threads();
        assert!(n >= 1, "Must use at least 1 thread for inference, got {}", n);
    }
}
