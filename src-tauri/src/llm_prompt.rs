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
         - Do NOT change the meaning or rephrase\n\
         - Do NOT add information that wasn't in the original\n\
         - Output language: {lang_name}\n\
         - Reply with ONLY the cleaned text, nothing else\n\
         - Do NOT use HTML, markdown, or any formatting\n\
         - Do NOT use <think> or reasoning tags"
    )
}
