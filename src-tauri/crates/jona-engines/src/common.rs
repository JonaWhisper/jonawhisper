use std::path::Path;

/// Punctuation labels predicted by the fullstop-punctuation model.
/// Index 0 = no punctuation, 1..5 = punctuation characters.
pub const PUNCT_LABELS: &[&str] = &["", ".", ",", "?", "-", ":"];

pub const WINDOW_SIZE: usize = 230;
pub const OVERLAP: usize = 5;

/// Strip existing punctuation and split into words.
pub fn strip_and_split(text: &str) -> Vec<String> {
    text.chars()
        .filter(|c| !matches!(c, '.' | ',' | '?' | ':' | '-' | ';' | '!'))
        .collect::<String>()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Windowed punctuation restoration: splits words into overlapping windows,
/// calls the inference function on each chunk, merges labels, and reconstructs text.
pub fn restore_punctuation_windowed<F>(text: &str, mut infer_chunk: F) -> Result<String, String>
where
    F: FnMut(&[String]) -> Result<Vec<usize>, String>,
{
    let words = strip_and_split(text);
    if words.is_empty() {
        return Ok(String::new());
    }

    let mut labels: Vec<usize> = vec![0; words.len()];
    let mut offset = 0;

    while offset < words.len() {
        let end = (offset + WINDOW_SIZE).min(words.len());
        let chunk = &words[offset..end];

        let chunk_labels = infer_chunk(chunk)?;

        // Merge: skip overlap words for non-first windows
        let start_word = if offset == 0 { 0 } else { OVERLAP };
        for (i, &label) in chunk_labels.iter().enumerate() {
            if i >= start_word {
                let global_idx = offset + i;
                if global_idx < words.len() {
                    labels[global_idx] = label;
                }
            }
        }

        if end >= words.len() {
            break;
        }
        offset += WINDOW_SIZE - OVERLAP;
    }

    // Reconstruct text with punctuation
    let mut result = String::new();
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        result.push_str(word);
        let label = labels[i];
        if label > 0 && label < PUNCT_LABELS.len() {
            result.push_str(PUNCT_LABELS[label]);
        }
    }

    Ok(result)
}

/// Download a file from a URL to a local path (blocking HTTP).
pub fn download_file(url: &str, path: &Path) -> Result<(), String> {
    log::info!("Downloading {} to {}", url, path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    let response = reqwest::blocking::get(url)
        .map_err(|e| format!("Failed to download {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Download failed with status {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response: {e}"))?;
    std::fs::write(path, &bytes).map_err(|e| format!("Failed to write file: {e}"))?;
    log::info!("Downloaded {} ({} bytes)", path.display(), bytes.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punct_labels_include_common_punctuation_marks() {
        // Punctuation restoration must handle periods, commas, question marks,
        // hyphens, and colons — the marks users expect in dictated text.
        assert!(PUNCT_LABELS.contains(&"."), "Must support periods");
        assert!(PUNCT_LABELS.contains(&","), "Must support commas");
        assert!(PUNCT_LABELS.contains(&"?"), "Must support question marks");
        assert!(PUNCT_LABELS.contains(&"-"), "Must support hyphens");
        assert!(PUNCT_LABELS.contains(&":"), "Must support colons");
        // Label 0 = no punctuation (the default)
        assert_eq!(PUNCT_LABELS[0], "");
    }

    #[test]
    fn strip_and_split_removes_punctuation() {
        let words = strip_and_split("Hello, world. How are you?");
        assert_eq!(words, vec!["Hello", "world", "How", "are", "you"]);
    }

    #[test]
    fn strip_and_split_handles_empty() {
        assert!(strip_and_split("").is_empty());
        assert!(strip_and_split("   ").is_empty());
    }

    #[test]
    fn strip_and_split_preserves_non_punct() {
        let words = strip_and_split("no punctuation here");
        assert_eq!(words, vec!["no", "punctuation", "here"]);
    }

    #[test]
    fn strip_and_split_removes_all_punct_types() {
        let words = strip_and_split("a. b, c? d: e- f; g!");
        assert_eq!(words, vec!["a", "b", "c", "d", "e", "f", "g"]);
    }

    #[test]
    fn restore_punctuation_windowed_empty() {
        let result = restore_punctuation_windowed("", |_chunk| Ok(vec![])).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn restore_punctuation_windowed_no_punct() {
        let result = restore_punctuation_windowed("hello world", |chunk| {
            Ok(vec![0; chunk.len()]) // label 0 = no punctuation
        }).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn restore_punctuation_windowed_adds_period() {
        let result = restore_punctuation_windowed("hello world", |chunk| {
            // Add period after last word
            let mut labels = vec![0; chunk.len()];
            if let Some(last) = labels.last_mut() {
                *last = 1; // "."
            }
            Ok(labels)
        }).unwrap();
        assert_eq!(result, "hello world.");
    }

    #[test]
    fn restore_punctuation_windowed_strips_existing_punct() {
        let result = restore_punctuation_windowed("hello, world.", |chunk| {
            // Return no punctuation — existing should be stripped
            Ok(vec![0; chunk.len()])
        }).unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn restore_punctuation_windowed_multiple_punct() {
        let result = restore_punctuation_windowed("is this a test", |chunk| {
            let mut labels = vec![0; chunk.len()];
            // "is this a test" → "is this a test?"
            // Label comma after "this" (index 1), question after "test" (index 3)
            labels[1] = 2; // ","
            labels[3] = 3; // "?"
            Ok(labels)
        }).unwrap();
        assert_eq!(result, "is this, a test?");
    }

    #[test]
    fn restore_punctuation_windowed_propagates_error() {
        let result = restore_punctuation_windowed("hello world", |_| {
            Err("inference failed".to_string())
        });
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("inference failed"));
    }

    #[test]
    fn window_size_exceeds_overlap() {
        // The sliding window must be larger than its overlap, otherwise
        // punctuation inference would never make forward progress.
        assert!(WINDOW_SIZE > OVERLAP,
            "Window size ({}) must exceed overlap ({})", WINDOW_SIZE, OVERLAP);
    }
}
