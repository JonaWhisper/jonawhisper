use regex::Regex;
use std::sync::LazyLock;

/// Known Whisper hallucination phrases that appear on silence/noise.
/// Organized by language, checked case-insensitively.
const HALLUCINATIONS: &[&str] = &[
    // -- Cross-language --
    "amara.org",
    "www.",
    "http",
    "\u{266A}",
    "\u{266B}",
    "...",
    "\u{2026}",
    // -- French --
    "sous-titrage soci\u{00E9}t\u{00E9} radio-canada",
    "sous-titrage st",
    "sous titrage soci\u{00E9}t\u{00E9} radio canada",
    "soustitrage soci\u{00E9}t\u{00E9} radio-canada",
    "sous-titrage",
    "sous-titres par",
    "sous-titres r\u{00E9}alis\u{00E9}s par",
    "par soustitreur.com",
    "merci d'avoir regard\u{00E9}",
    "merci pour votre \u{00E9}coute",
    "au revoir.",
    "\u{00E0} bient\u{00F4}t.",
    // -- English --
    "subtitles by",
    "thank you for watching",
    "thanks for watching",
    "please subscribe",
    "like and subscribe",
    "don't forget to subscribe",
    "see you in the next video",
    "bye.",
    "bye bye.",
    "bye-bye.",
    // -- German --
    "untertitel im auftrag des zdf",
    "untertitel der amara.org-community",
    "vielen dank f\u{00FC}rs zuschauen",
    "danke f\u{00FC}rs zuschauen",
    "bis zum n\u{00E4}chsten mal",
    "tsch\u{00FC}ss",
    // -- Spanish --
    "subt\u{00ED}tulos realizados por",
    "subtitulado por",
    "gracias por ver",
    "suscr\u{00ED}bete al canal",
    "no olvides suscribirte",
    // -- Portuguese --
    "legendas pela comunidade",
    "obrigado por assistir",
    "tchau",
    // -- Italian --
    "sottotitoli creati dalla comunit\u{00E0}",
    "sottotitoli a cura di",
    "grazie per la visione",
    "grazie per aver guardato",
    // -- Dutch --
    "ondertiteld door",
    "ondertiteling door",
    "bedankt voor het kijken",
    // -- Polish --
    "napisy stworzone przez",
    "dzi\u{0119}kuj\u{0119} za obejrzenie",
    "dzi\u{0119}kuj\u{0119} za uwag\u{0119}",
    // -- Russian --
    "\u{0441}\u{0443}\u{0431}\u{0442}\u{0438}\u{0442}\u{0440}\u{044B} \u{0441}\u{0434}\u{0435}\u{043B}\u{0430}\u{043D}\u{044B} \u{0441}\u{043E}\u{043E}\u{0431}\u{0449}\u{0435}\u{0441}\u{0442}\u{0432}\u{043E}\u{043C}",
    "\u{0441}\u{043F}\u{0430}\u{0441}\u{0438}\u{0431}\u{043E} \u{0437}\u{0430} \u{043F}\u{0440}\u{043E}\u{0441}\u{043C}\u{043E}\u{0442}\u{0440}",
    "\u{043F}\u{043E}\u{0434}\u{043F}\u{0438}\u{0441}\u{044B}\u{0432}\u{0430}\u{0439}\u{0442}\u{0435}\u{0441}\u{044C} \u{043D}\u{0430} \u{043A}\u{0430}\u{043D}\u{0430}\u{043B}",
];

// Pre-compiled regexes for hallucination removal (case-insensitive)
static HALLUCINATION_REGEXES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    HALLUCINATIONS
        .iter()
        .map(|h| Regex::new(&format!("(?i){}", regex::escape(h))).unwrap())
        .collect()
});

// Music/symbol-only output
static RE_MUSIC_ONLY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\s\u{266A}\u{266B}\u{1F3B5}.\u{2026}]+$").unwrap());

/// Strip known Whisper hallucination phrases from text.
/// If only hallucinations remain, returns empty string.
pub(super) fn strip_hallucinations(text: &str) -> String {
    let mut result = text.to_string();
    let lower = result.to_lowercase();
    let trimmed_lower = lower.trim().trim_matches('.').trim();

    // Music/symbol-only output
    if RE_MUSIC_ONLY.is_match(trimmed_lower) {
        log::info!("Filtered hallucination (music/symbols): {:?}", text.trim());
        return String::new();
    }

    // If the entire text (trimmed, case-insensitive) matches a hallucination, discard it
    for h in HALLUCINATIONS {
        if trimmed_lower == *h || trimmed_lower.starts_with(h) {
            log::info!("Filtered hallucination: {:?}", text.trim());
            return String::new();
        }
    }

    // Repetition detection: same word 3+ times in a row → likely looping
    if has_excessive_repetition(trimmed_lower) {
        log::info!("Filtered hallucination (repetition): {:?}", text.trim());
        return String::new();
    }

    // Remove hallucination phrases embedded in longer text
    for re in HALLUCINATION_REGEXES.iter() {
        result = re.replace_all(&result, "").to_string();
    }

    result
}

/// Detect excessive repetition (same word 3+ times in a row, or text is mostly one word).
/// Whisper hallucinates by looping the same word/phrase on silence.
fn has_excessive_repetition(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 3 {
        return false;
    }

    // Check for same word repeated 3+ times consecutively
    let mut run_count = 1;
    for i in 1..words.len() {
        if words[i].eq_ignore_ascii_case(words[i - 1]) {
            run_count += 1;
            if run_count >= 3 {
                return true;
            }
        } else {
            run_count = 1;
        }
    }

    // Check if a single word dominates (>70% of all words, at least 4 occurrences)
    let mut counts = std::collections::HashMap::new();
    for w in &words {
        *counts.entry(w.to_lowercase()).or_insert(0u32) += 1;
    }
    if let Some(&max_count) = counts.values().max() {
        if max_count >= 4 && (max_count as f32 / words.len() as f32) > 0.7 {
            return true;
        }
    }

    false
}
