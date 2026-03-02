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

/// Download a file from a URL to a local path.
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
