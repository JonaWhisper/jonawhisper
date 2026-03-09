pub mod itn;
pub mod llm_cloud;
pub mod post_processor;
pub mod symspell_correct;
pub mod vad;

pub use jona_engines::llm_prompt::LlmError;

/// Full text-only cleanup pipeline: preprocess → ITN → finalize.
/// This chains the text transformations that don't require ML models.
/// ML-based steps (punctuation, correction, LLM) are applied separately.
#[allow(dead_code)]
pub fn text_cleanup_pipeline(
    raw_asr: &str,
    language: &str,
    hallucination_filter: bool,
    disfluency_removal: bool,
    itn_enabled: bool,
) -> String {
    let opts = post_processor::PostProcessOptions {
        hallucination_filter,
        disfluency_removal,
        dictation_commands: true,
    };

    let preprocessed = post_processor::preprocess(raw_asr, language, &opts);
    if preprocessed.is_empty() {
        return String::new();
    }

    let after_itn = if itn_enabled {
        itn::apply_itn(&preprocessed, language)
    } else {
        preprocessed
    };

    post_processor::finalize(&after_itn)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Full pipeline: raw ASR output → clean text ready for paste
    // These test the BEHAVIOR a user expects: speak → get clean text
    // =========================================================================

    #[test]
    fn dictated_french_sentence_with_numbers_and_punctuation() {
        // User says: "j'ai vingt-trois ans virgule et je pèse soixante-dix kilos point"
        let raw = "j'ai vingt-trois ans virgule et je pèse soixante-dix kilos point";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert_eq!(result, "J'ai 23 ans, et je p\u{00e8}se 70 kg.");
    }

    #[test]
    fn dictated_english_sentence_with_numbers() {
        let raw = "I have twenty three cats and five dogs";
        let result = text_cleanup_pipeline(raw, "en", true, true, true);
        assert_eq!(result, "I have 23 cats and 5 dogs");
    }

    #[test]
    fn hallucination_produces_empty_output() {
        // Whisper hallucinates on silence — pipeline should return empty
        let raw = "Sous-titrage Société Radio-Canada";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert!(result.is_empty(), "Hallucination should be filtered to empty, got: {}", result);
    }

    #[test]
    fn hallucination_filter_disabled_keeps_text() {
        let raw = "sous-titrage";
        let result = text_cleanup_pipeline(raw, "fr", false, true, true);
        assert!(!result.is_empty());
    }

    #[test]
    fn fillers_removed_from_french_speech() {
        // User hesitates while speaking
        let raw = "euh je veux euh aller au magasin";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert!(!result.to_lowercase().contains("euh"));
        assert!(result.contains("magasin"));
    }

    #[test]
    fn fillers_removed_from_english_speech() {
        let raw = "um I want to uh go to the store";
        let result = text_cleanup_pipeline(raw, "en", true, true, true);
        assert!(!result.to_lowercase().contains("um"));
        assert!(!result.to_lowercase().contains("uh"));
        assert!(result.contains("store"));
    }

    #[test]
    fn fillers_kept_when_disabled() {
        let raw = "euh bonjour";
        let result = text_cleanup_pipeline(raw, "fr", true, false, true);
        assert!(result.to_lowercase().contains("euh"));
    }

    #[test]
    fn dictation_commands_replaced_in_french() {
        let raw = "bonjour point d'exclamation à la ligne comment allez-vous point d'interrogation";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert!(result.contains('!'), "Should have exclamation: {}", result);
        assert!(result.contains('\n'), "Should have newline: {}", result);
        assert!(result.contains('?'), "Should have question mark: {}", result);
    }

    #[test]
    fn dictation_commands_replaced_in_english() {
        let raw = "hello exclamation mark new line how are you question mark";
        let result = text_cleanup_pipeline(raw, "en", true, true, true);
        assert!(result.contains('!'));
        assert!(result.contains('\n'));
        assert!(result.contains('?'));
    }

    #[test]
    fn itn_converts_currencies_in_pipeline() {
        let raw = "ça coûte cinquante euros";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert!(result.contains("50"), "Should convert number: {}", result);
        assert!(result.contains('\u{20AC}'), "Should convert euro: {}", result);
    }

    #[test]
    fn itn_disabled_keeps_words() {
        let raw = "j'ai cinq chats";
        let result = text_cleanup_pipeline(raw, "fr", true, true, false);
        assert!(result.contains("cinq"), "ITN disabled should keep word 'cinq': {}", result);
        assert!(!result.contains('5'), "ITN disabled should not convert to digit");
    }

    #[test]
    fn itn_converts_hours_in_french() {
        let raw = "il est trois heures et demie";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert!(result.contains("3"), "Should convert 'trois': {}", result);
        assert!(result.contains("h"), "Should have 'h' for heures: {}", result);
        assert!(result.contains("30"), "Should convert 'et demie': {}", result);
    }

    #[test]
    fn itn_converts_percentages_in_english() {
        let raw = "twenty percent discount";
        let result = text_cleanup_pipeline(raw, "en", true, true, true);
        assert!(result.contains("20"), "Should convert number: {}", result);
        assert!(result.contains('%'), "Should convert percent: {}", result);
    }

    #[test]
    fn itn_converts_ordinals_in_french() {
        let raw = "le premier janvier";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert!(result.contains("1er"), "Should convert ordinal: {}", result);
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(text_cleanup_pipeline("", "fr", true, true, true).is_empty());
        assert!(text_cleanup_pipeline("   ", "en", true, true, true).is_empty());
    }

    #[test]
    fn first_letter_capitalized() {
        let raw = "bonjour le monde";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert!(result.starts_with('B'), "First letter should be capitalized: {}", result);
    }

    #[test]
    fn capitalization_after_sentence_end() {
        let raw = "bonjour. comment ça va";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        // After ". " the next word should be capitalized
        assert!(result.contains(". C") || result.contains(". \u{00C7}"),
            "Should capitalize after period: {}", result);
    }

    #[test]
    fn auto_language_detection_french() {
        // Enough French words to trigger FR detection
        let raw = "le chat est dans la maison et il mange";
        let result = text_cleanup_pipeline(raw, "auto", true, true, true);
        assert!(result.starts_with("Le"));
    }

    #[test]
    fn complex_french_dictation_full_pipeline() {
        // Realistic dictation with fillers, commands, numbers, and units
        let raw = "euh la température est de vingt degrés virgule euh il fait beau point";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        // Fillers removed
        assert!(!result.to_lowercase().contains("euh"));
        // Number converted
        assert!(result.contains("20"), "Should have 20: {}", result);
        // Unit converted
        assert!(result.contains('\u{00B0}'), "Should have degree symbol: {}", result);
        // Punctuation from dictation commands
        assert!(result.contains(','), "Should have comma from 'virgule': {}", result);
        assert!(result.contains('.'), "Should have period from 'point': {}", result);
    }

    #[test]
    fn embedded_hallucination_removed_rest_kept() {
        let raw = "bonjour sous-titrage tout le monde";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert!(result.contains("Bonjour"));
        assert!(!result.to_lowercase().contains("sous-titrage"));
    }

    #[test]
    fn units_converted_in_english_pipeline() {
        let raw = "the distance is five kilometers";
        let result = text_cleanup_pipeline(raw, "en", true, true, true);
        assert!(result.contains("5"), "Number converted: {}", result);
        assert!(result.contains("km"), "Unit converted: {}", result);
    }

    #[test]
    fn large_number_conversion() {
        let raw = "il y a deux millions d'habitants";
        let result = text_cleanup_pipeline(raw, "fr", true, true, true);
        assert!(result.contains("2000000"), "Large number converted: {}", result);
    }
}
