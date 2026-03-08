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
        log::info!("LLM flagged input as hallucination");
        return Err(LlmError::Hallucination);
    }

    if result.is_empty() {
        log::warn!("LLM returned empty output (input_len={})", input_len);
        return Err(LlmError::InvalidResponse("Empty output".into()));
    }

    // Only reject if output is unreasonably LONG (expansion beyond 3x is suspicious)
    let max_len = std::cmp::max(input_len * 3, 200);
    if result.len() > max_len {
        log::warn!("LLM output too long (len={} vs input={}, max={}), rejecting", result.len(), input_len, max_len);
        return Err(LlmError::InvalidResponse(format!(
            "Output too long (len={} vs input={})", result.len(), input_len
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

#[cfg(test)]
mod tests {
    use super::*;

    // -- strip_think_blocks --

    #[test]
    fn strip_think_blocks_no_blocks() {
        assert_eq!(strip_think_blocks("Hello world"), "Hello world");
    }

    #[test]
    fn strip_think_blocks_single_block() {
        assert_eq!(
            strip_think_blocks("before<think>reasoning</think>after"),
            "beforeafter"
        );
    }

    #[test]
    fn strip_think_blocks_multiple_blocks() {
        assert_eq!(
            strip_think_blocks("a<think>x</think>b<think>y</think>c"),
            "abc"
        );
    }

    #[test]
    fn strip_think_blocks_unclosed() {
        // Unclosed think block discards the rest
        assert_eq!(strip_think_blocks("before<think>unclosed"), "before");
    }

    #[test]
    fn strip_think_blocks_empty_block() {
        assert_eq!(strip_think_blocks("a<think></think>b"), "ab");
    }

    // -- sanitize_output --

    #[test]
    fn sanitize_output_normal() {
        let result = sanitize_output("Hello world", 11).unwrap();
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn sanitize_output_trims_whitespace() {
        let result = sanitize_output("  Hello world  ", 11).unwrap();
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn sanitize_output_strips_think_blocks() {
        let result = sanitize_output("<think>reasoning</think>Cleaned text", 20).unwrap();
        assert_eq!(result, "Cleaned text");
    }

    #[test]
    fn sanitize_output_hallucination() {
        let result = sanitize_output("HALLUCINATION", 10);
        assert!(matches!(result, Err(LlmError::Hallucination)));
    }

    #[test]
    fn sanitize_output_empty() {
        let result = sanitize_output("", 10);
        assert!(result.is_err());
    }

    #[test]
    fn sanitize_output_empty_after_think_strip() {
        let result = sanitize_output("<think>all reasoning no output</think>", 10);
        assert!(result.is_err());
    }

    #[test]
    fn sanitize_output_too_long() {
        let input_len = 10;
        // max_len = max(10*3, 200) = 200
        let long_output = "a".repeat(201);
        let result = sanitize_output(&long_output, input_len);
        assert!(result.is_err());
    }

    #[test]
    fn sanitize_output_just_within_limit() {
        let input_len = 10;
        // max_len = max(10*3, 200) = 200
        let output = "a".repeat(200);
        let result = sanitize_output(&output, input_len);
        assert!(result.is_ok());
    }

    #[test]
    fn sanitize_output_large_input_scales_limit() {
        let input_len = 100;
        // max_len = max(100*3, 200) = 300
        let output = "a".repeat(300);
        let result = sanitize_output(&output, input_len);
        assert!(result.is_ok());

        let output = "a".repeat(301);
        let result = sanitize_output(&output, input_len);
        assert!(result.is_err());
    }

    // -- system_prompt --

    #[test]
    fn system_prompt_french() {
        let prompt = system_prompt("fr");
        assert!(prompt.contains("French"));
        assert!(prompt.contains("dictation"));
    }

    #[test]
    fn system_prompt_english() {
        let prompt = system_prompt("en");
        assert!(prompt.contains("English"));
    }

    #[test]
    fn system_prompt_unknown_language() {
        let prompt = system_prompt("ja");
        assert!(prompt.contains("the same language as the input"));
    }

    #[test]
    fn system_prompt_contains_hallucination_instruction() {
        let prompt = system_prompt("en");
        assert!(prompt.contains("HALLUCINATION"));
    }
}
