use super::downloader::write_version_json;
use super::ASRModel;
use std::fs;

/// Migrate old download markers (.complete_v2, .complete_v3) to .complete.
/// Should be called once at startup before check_model_updates.
pub fn migrate_download_markers() {
    let correction_dir = jona_types::models_dir().join("correction");
    if !correction_dir.is_dir() {
        return;
    }

    let entries = match fs::read_dir(&correction_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        for old_marker in &[".complete_v2", ".complete_v3"] {
            let old_path = path.join(old_marker);
            if old_path.exists() {
                let new_path = path.join(".complete");
                match fs::rename(&old_path, &new_path) {
                    Ok(()) => log::info!("Migrated {} -> .complete in {}", old_marker, path.display()),
                    Err(e) => log::warn!("Failed to migrate {} in {}: {}", old_marker, path.display(), e),
                }
            }
        }
    }
}

/// Migrate existing version.json files to include ETags.
/// For downloaded models whose version.json is missing ETags, re-generates
/// the file with URL + ETag + SHA256 using the current catalog URLs.
pub fn migrate_version_json(downloaded_models: &[ASRModel]) {
    for model in downloaded_models {
        let model_path = model.local_path();
        let version_dir = if model_path.is_dir() {
            model_path
        } else {
            match model_path.parent() {
                Some(p) => p.to_path_buf(),
                None => continue,
            }
        };

        let version_path = version_dir.join("version.json");

        // Skip if no version.json at all (will be created on next download)
        if !version_path.exists() {
            continue;
        }

        // Check if it already has etag(s)
        let content = match fs::read_to_string(&version_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let local: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let has_etags = if let Some(files) = local.get("files").and_then(|v| v.as_object()) {
            files.values().any(|e| e.get("etag").is_some())
        } else {
            local.get("etag").is_some()
        };

        if has_etags {
            continue;
        }

        log::info!("Migrating version.json for {} (adding ETags)", model.id);
        write_version_json(model);
    }
}
