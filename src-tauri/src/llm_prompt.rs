/// Shared error type for all LLM operations (local and cloud).
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("LLM not configured")]
    NotConfigured,
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error: {status} {body}")]
    Api { status: u16, body: String },
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Inference error: {0}")]
    Inference(String),
    #[error("Hallucination detected")]
    Hallucination,
}

/// Strip `<think>...</think>` blocks emitted by reasoning models (e.g. Qwen3, DeepSeek).
fn strip_think_blocks(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<think>") {
        result.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find("</think>") {
            rest = &rest[start + end + "</think>".len()..];
        } else {
            // Unclosed <think> — discard the rest
            return result;
        }
    }
    result.push_str(rest);
    result
}

/// Sanity-check LLM output: strip think blocks, detect hallucination rejection,
/// reject empty or unreasonably long results.
pub fn sanitize_output(raw: &str, input_len: usize) -> Result<String, LlmError> {
    let cleaned = strip_think_blocks(raw);
    let result = cleaned.trim().to_string();

    // LLM detected hallucinated input
    if result == "HALLUCINATION" {
        log::info!("LLM flagged input as hallucination, discarding");
        return Err(LlmError::Hallucination);
    }

    let max_len = std::cmp::max(input_len * 3, 100);
    if result.is_empty() || result.len() > max_len {
        log::warn!("LLM output suspicious (len={} vs input={}, max={}), discarding", result.len(), input_len, max_len);
        return Err(LlmError::InvalidResponse(format!(
            "Output failed sanity check (len={} vs input={})", result.len(), input_len
        )));
    }
    Ok(result)
}

/// Shared system prompt for LLM text cleanup (used by both local and cloud paths).
pub fn system_prompt(language: &str) -> String {
    let lang_name = match language {
        "fr" => "French",
        "en" => "English",
        "es" => "Spanish",
        "de" => "German",
        _ => "the same language as the input",
    };

    format!(
        "/no_think\n\
         You are a dictation text cleaner. Your job is to clean up raw speech-to-text output.\n\
         Rules:\n\
         - Fix punctuation, capitalization, and spacing\n\
         - Remove filler words and speech artifacts (um, uh, etc.)\n\
         - If the input looks like hallucinated speech-to-text (repetitive phrases, generic subtitles like \"Thanks for watching\", \"Subscribe\", or text clearly not from real dictation), reply with ONLY the word HALLUCINATION\n\
         - Do NOT change the meaning or rephrase\n\
         - Do NOT add information that wasn't in the original\n\
         - Output language: {lang_name}\n\
         - Reply with ONLY the cleaned text, nothing else\n\
         - Do NOT use HTML, markdown, or any formatting\n\
         - Do NOT use <think> or reasoning tags"
    )
}
