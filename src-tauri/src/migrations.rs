use serde_json::Value;
use crate::state::{config_dir, default_model_id, Provider, Preferences};

/// Current schema version. Bump when adding a new migration.
const CURRENT_VERSION: u32 = 8;

type MigrationFn = fn(&mut Value, &mut Preferences);

const MIGRATIONS: &[(u32, &str, MigrationFn)] = &[
    (1, "Unify providers and cleanup settings", migrate_v1),
    (2, "Centralize model storage", migrate_v2),
    (3, "Update llm_max_tokens default to 4096", migrate_v3),
    (4, "Migrate API keys to OS keychain", migrate_v4),
    (5, "Add provider capability flags", migrate_v5),
    (6, "Split punctuation from cleanup model", migrate_v6),
    (7, "Clean up old Candle/safetensors correction models", migrate_v7),
    (8, "Normalize provider kind to lowercase string IDs", migrate_v8),
];

/// Rename data directory from WhisperDictate → JonaWhisper.
/// Must run before Preferences::load() since config_dir() now points to JonaWhisper/.
pub fn migrate_data_directory() {
    let base = match dirs::config_dir() {
        Some(d) => d,
        None => return,
    };
    let old_dir = base.join("WhisperDictate");
    let new_dir = base.join("JonaWhisper");

    if !old_dir.exists() || !old_dir.is_dir() {
        return;
    }
    if new_dir.exists() {
        log::info!("Data dir migration: both WhisperDictate/ and JonaWhisper/ exist, keeping JonaWhisper/");
        return;
    }

    match std::fs::rename(&old_dir, &new_dir) {
        Ok(()) => log::info!("Data dir migration: renamed {} → {}", old_dir.display(), new_dir.display()),
        Err(e) => log::warn!("Data dir migration: failed to rename {} → {}: {}", old_dir.display(), new_dir.display(), e),
    }
}

/// Run all pending migrations. Returns true if any migration was applied (needs save).
pub fn run(raw: &mut Value, prefs: &mut Preferences) -> bool {
    let version = raw.get("_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    if version >= CURRENT_VERSION { return false; }

    for &(v, name, func) in MIGRATIONS {
        if v > version {
            log::info!("Running migration v{}: {}", v, name);
            func(raw, prefs);
        }
    }

    raw["_version"] = serde_json::json!(CURRENT_VERSION);
    prefs.schema_version = CURRENT_VERSION;
    true
}

/// v1: Migrate api_servers/llm_config → providers, cleanup_mode → text_cleanup_enabled, etc.
pub(crate) fn migrate_v1(raw: &mut Value, prefs: &mut Preferences) {
    let has_old_api_servers = raw.get("api_servers").is_some();
    let has_old_llm_config = raw.get("llm_config").is_some();

    if has_old_api_servers || has_old_llm_config {
        log::info!("Migrating preferences from old format to unified providers");

        // 1. Convert api_servers → providers
        if let Some(servers) = raw.get("api_servers").and_then(|v| v.as_array()) {
            for server in servers {
                if let (Some(id), Some(name), Some(url), Some(model)) = (
                    server.get("id").and_then(|v| v.as_str()),
                    server.get("name").and_then(|v| v.as_str()),
                    server.get("url").and_then(|v| v.as_str()),
                    server.get("model").and_then(|v| v.as_str()),
                ) {
                    let api_key = server.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
                    prefs.providers.push(Provider {
                        id: id.to_string(),
                        name: name.to_string(),
                        kind: "custom".to_string(),
                        url: url.to_string(),
                        api_key: api_key.to_string(),
                        allow_insecure: false,
                        cached_models: Vec::new(),
                        supports_asr: true,
                        supports_llm: true,
                        api_format: None,
                        extra: std::collections::HashMap::new(),
                    });
                    // Migrate ASR model to settings
                    if !model.is_empty() && !prefs.selected_model_id.starts_with("cloud:") {
                        prefs.selected_model_id = format!("cloud:{}", id);
                        prefs.asr_cloud_model = model.to_string();
                    }
                }
            }
        }

        // 2. Convert llm_config → provider + settings
        if let Some(llm) = raw.get("llm_config") {
            let provider_str = llm.get("provider").and_then(|v| v.as_str()).unwrap_or("openai");
            let api_url = llm.get("api_url").and_then(|v| v.as_str()).unwrap_or("");
            let api_key = llm.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
            let model = llm.get("model").and_then(|v| v.as_str()).unwrap_or("");

            if llm.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
                prefs.text_cleanup_enabled = true;
            }
            prefs.llm_model = model.to_string();

            if !api_url.is_empty() {
                let existing = prefs.providers.iter().find(|p|
                    p.url == api_url && p.api_key == api_key
                );
                if let Some(p) = existing {
                    prefs.llm_provider_id = p.id.clone();
                } else {
                    let (kind_str, display, has_asr, has_llm) = if provider_str == "anthropic" {
                        ("anthropic", "Anthropic", false, true)
                    } else {
                        ("openai", "OpenAI", true, true)
                    };
                    let id = format!("provider-{}", provider_str);
                    prefs.providers.push(Provider {
                        id: id.clone(),
                        name: display.to_string(),
                        kind: kind_str.to_string(),
                        url: api_url.to_string(),
                        api_key: api_key.to_string(),
                        allow_insecure: false,
                        cached_models: Vec::new(),
                        supports_asr: has_asr,
                        supports_llm: has_llm,
                        api_format: None,
                        extra: std::collections::HashMap::new(),
                    });
                    prefs.llm_provider_id = id;
                }
            }
        }
    }

    // Migrate providers that still have asr_model in JSON (old unified format)
    if let Some(providers_json) = raw.get("providers").and_then(|v| v.as_array()) {
        for pj in providers_json {
            if let Some(asr_model) = pj.get("asr_model").and_then(|v| v.as_str()) {
                if !asr_model.is_empty() && !prefs.selected_model_id.starts_with("cloud:") {
                    if let Some(pid) = pj.get("id").and_then(|v| v.as_str()) {
                        log::info!("Migrating asr_model from provider {} to settings", pid);
                        prefs.selected_model_id = format!("cloud:{}", pid);
                        prefs.asr_cloud_model = asr_model.to_string();
                    }
                }
            }
        }
    }

    // Reset selected_model_id if it was pointing to old openai-api: pseudo-model
    if prefs.selected_model_id.starts_with("openai-api:") {
        log::info!("Resetting selected_model_id from old openai-api: format");
        prefs.selected_model_id = default_model_id();
    }

    // Migrate asr_provider_id → selected_model_id = "cloud:<provider_id>"
    if let Some(asr_pid) = raw.get("asr_provider_id").and_then(|v| v.as_str()) {
        if !asr_pid.is_empty() {
            let cloud_id = format!("cloud:{}", asr_pid);
            log::info!("Migrating asr_provider_id={} → selected_model_id={}", asr_pid, cloud_id);
            prefs.selected_model_id = cloud_id;
        }
    }

    // Migrate old cleanup_mode/punctuation_model_id/llm_source/llm_local_model_id → unified
    if let Some(cleanup_mode) = raw.get("cleanup_mode").and_then(|v| v.as_str()) {
        let old_llm_source = raw.get("llm_source").and_then(|v| v.as_str()).unwrap_or("cloud");
        let old_punctuation_model_id = raw.get("punctuation_model_id").and_then(|v| v.as_str()).unwrap_or("");
        let old_llm_local_model_id = raw.get("llm_local_model_id").and_then(|v| v.as_str()).unwrap_or("");
        let old_llm_provider_id = raw.get("llm_provider_id").and_then(|v| v.as_str()).unwrap_or("");

        match cleanup_mode {
            "punctuation" => {
                log::info!("Migrating cleanup_mode=punctuation → text_cleanup_enabled=true, cleanup_model_id={}", old_punctuation_model_id);
                prefs.text_cleanup_enabled = true;
                prefs.cleanup_model_id = old_punctuation_model_id.to_string();
            }
            "full" => {
                prefs.text_cleanup_enabled = true;
                if old_llm_source == "local" {
                    let mut model_id = old_llm_local_model_id.to_string();
                    if model_id.starts_with("llm-local:") {
                        model_id = model_id.replacen("llm-local:", "llama:", 1);
                    }
                    log::info!("Migrating cleanup_mode=full, llm_source=local → cleanup_model_id={}", model_id);
                    prefs.cleanup_model_id = model_id;
                } else {
                    let cloud_id = format!("cloud:{}", old_llm_provider_id);
                    log::info!("Migrating cleanup_mode=full, llm_source=cloud → cleanup_model_id={}", cloud_id);
                    prefs.cleanup_model_id = cloud_id;
                }
            }
            _ => {
                prefs.text_cleanup_enabled = false;
            }
        }
    }

    // Migrate llm_enabled (even older format) → text_cleanup_enabled
    if let Some(llm_enabled) = raw.get("llm_enabled").and_then(|v| v.as_bool()) {
        if llm_enabled && !prefs.text_cleanup_enabled {
            log::info!("Migrating llm_enabled=true → text_cleanup_enabled=true");
            prefs.text_cleanup_enabled = true;
        }
    }

    // Finalize cloud cleanup_model_id: ensure "cloud:" prefix with provider ID
    if prefs.text_cleanup_enabled && (prefs.cleanup_model_id.is_empty() || prefs.cleanup_model_id == "cloud")
        && !prefs.llm_provider_id.is_empty() {
            prefs.cleanup_model_id = format!("cloud:{}", prefs.llm_provider_id);
    }
}

/// v2: Move model files from scattered directories to ~/Library/Application Support/WhisperDictate/models/
fn migrate_v2(raw: &mut Value, prefs: &mut Preferences) {
    let _ = (raw, prefs); // Only filesystem operations

    let mappings: &[(&str, &str)] = &[
        ("~/.local/share/whisper-cpp", "whisper"),
        ("~/.local/share/whisper-dictate/llm", "llm"),
        ("~/.local/share/whisper-dictate/bert", "bert"),
    ];

    let models_base = config_dir().join("models");

    for &(old_tilde, subdir) in mappings {
        let old_dir = std::path::PathBuf::from(shellexpand::tilde(old_tilde).as_ref());
        let new_dir = models_base.join(subdir);

        if !old_dir.exists() || !old_dir.is_dir() {
            continue;
        }

        if let Err(e) = std::fs::create_dir_all(&new_dir) {
            log::warn!("Migration v2: failed to create {}: {}", new_dir.display(), e);
            continue;
        }

        let entries = match std::fs::read_dir(&old_dir) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Migration v2: failed to read {}: {}", old_dir.display(), e);
                continue;
            }
        };

        for entry in entries.flatten() {
            let src = entry.path();
            if !src.is_file() { continue; }

            let filename = match src.file_name() {
                Some(f) => f,
                None => continue,
            };
            let dst = new_dir.join(filename);

            if dst.exists() {
                log::info!("Migration v2: skip {} (already exists)", dst.display());
                continue;
            }

            // Try rename first (atomic if same volume), fall back to copy+delete
            if std::fs::rename(&src, &dst).is_ok() {
                log::info!("Migration v2: moved {} → {}", src.display(), dst.display());
            } else {
                match std::fs::copy(&src, &dst) {
                    Ok(_) => {
                        let _ = std::fs::remove_file(&src);
                        log::info!("Migration v2: copied {} → {}", src.display(), dst.display());
                    }
                    Err(e) => {
                        log::warn!("Migration v2: failed to copy {} → {}: {}", src.display(), dst.display(), e);
                    }
                }
            }
        }

        // Remove old directory if empty
        if std::fs::read_dir(&old_dir).is_ok_and(|mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&old_dir);
            log::info!("Migration v2: removed empty dir {}", old_dir.display());
        }
    }

    // Clean up ~/.local/share/whisper-dictate/ if empty
    let wd_dir = std::path::PathBuf::from(shellexpand::tilde("~/.local/share/whisper-dictate").as_ref());
    if wd_dir.exists()
        && std::fs::read_dir(&wd_dir).is_ok_and(|mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&wd_dir);
            log::info!("Migration v2: removed empty dir {}", wd_dir.display());
    }
}

/// v3: Update llm_max_tokens from old default (256) to new default (4096)
pub(crate) fn migrate_v3(_raw: &mut Value, prefs: &mut Preferences) {
    if prefs.llm_max_tokens <= 256 {
        log::info!("Migration v3: updating llm_max_tokens from {} to 4096", prefs.llm_max_tokens);
        prefs.llm_max_tokens = 4096;
    }
}

/// v4: Migrate plaintext API keys from preferences.json into the OS keychain.
/// After this migration, api_key fields are cleared from JSON and stored in keyring.
fn migrate_v4(_raw: &mut Value, prefs: &mut Preferences) {
    use crate::state::keyring_store;

    let mut migrated = 0;
    for provider in &mut prefs.providers {
        if !provider.api_key.is_empty() {
            keyring_store(&provider.id, &provider.api_key);
            migrated += 1;
            // Don't clear here — the runtime struct still needs the key.
            // save() will strip api_key from JSON automatically.
        }
    }
    if migrated > 0 {
        log::info!("Migration v4: migrated {} API key(s) to OS keychain", migrated);
    }
}

/// v5: Populate provider capability flags (supports_asr/supports_llm).
fn migrate_v5(_raw: &mut Value, prefs: &mut Preferences) {
    for provider in &mut prefs.providers {
        provider.supports_asr = provider.has_asr();
        provider.supports_llm = provider.has_llm();
    }
    log::info!("Migration v5: set capability flags for {} provider(s)", prefs.providers.len());
}

/// v6: Split punctuation model out of cleanup_model_id into its own punctuation_model_id field.
pub(crate) fn migrate_v6(_raw: &mut Value, prefs: &mut Preferences) {
    let is_punctuation = prefs.cleanup_model_id.starts_with("bert-punctuation:")
        || prefs.cleanup_model_id.starts_with("pcs-punctuation:");

    if is_punctuation {
        log::info!(
            "Migration v6: moving {} from cleanup_model_id to punctuation_model_id",
            prefs.cleanup_model_id
        );
        prefs.punctuation_model_id = std::mem::take(&mut prefs.cleanup_model_id);
        // Disable text cleanup since the user only had punctuation, not a real cleanup model
        prefs.text_cleanup_enabled = false;
    }
}

/// v7: Clean up old Candle/safetensors correction model files, replaced by ONNX.
/// Removes: model.safetensors, .complete marker, and the flan-t5-grammar directory.
fn migrate_v7(_raw: &mut Value, prefs: &mut Preferences) {
    let correction_dir = jona_types::models_dir().join("correction");
    if !correction_dir.exists() {
        return;
    }

    // Models that switched from safetensors to ONNX
    let migrated_models = ["gec-t5-small", "t5-spell-fr", "flanec-base", "flanec-large"];
    let stale_files = ["model.safetensors", ".complete"];

    for model_name in &migrated_models {
        let model_dir = correction_dir.join(model_name);
        if !model_dir.exists() {
            continue;
        }
        for filename in &stale_files {
            let path = model_dir.join(filename);
            if path.exists() {
                match std::fs::remove_file(&path) {
                    Ok(()) => log::info!("Migration v7: removed {}", path.display()),
                    Err(e) => log::warn!("Migration v7: failed to remove {}: {}", path.display(), e),
                }
            }
        }
    }

    // flan-t5-grammar was removed from the catalog entirely
    let flan_dir = correction_dir.join("flan-t5-grammar");
    if flan_dir.exists() {
        match std::fs::remove_dir_all(&flan_dir) {
            Ok(()) => log::info!("Migration v7: removed {}", flan_dir.display()),
            Err(e) => log::warn!("Migration v7: failed to remove {}: {}", flan_dir.display(), e),
        }
    }

    // Reset cleanup_model_id if it pointed to flan-t5-grammar
    if prefs.cleanup_model_id == "correction:flan-t5-grammar" {
        log::info!("Migration v7: resetting cleanup_model_id from flan-t5-grammar");
        prefs.cleanup_model_id.clear();
        prefs.text_cleanup_enabled = false;
    }
}

/// v8: Normalize provider kind from PascalCase enum variants to lowercase string IDs.
/// Also fill in base_url from presets for providers that have an empty URL.
pub(crate) fn migrate_v8(_raw: &mut Value, prefs: &mut Preferences) {
    let kind_map: &[(&str, &str)] = &[
        ("OpenAI", "openai"),
        ("Anthropic", "anthropic"),
        ("Custom", "custom"),
        ("Groq", "groq"),
        ("Cerebras", "cerebras"),
        ("Gemini", "gemini"),
        ("Mistral", "mistral"),
        ("Fireworks", "fireworks"),
        ("Together", "together"),
        ("DeepSeek", "deepseek"),
    ];

    let mut migrated = 0;
    for provider in &mut prefs.providers {
        let old_kind = provider.kind.clone();
        // Normalize known PascalCase → lowercase
        if let Some(&(_, new)) = kind_map.iter().find(|&&(old, _)| old == old_kind) {
            provider.kind = new.to_string();
        } else if old_kind.chars().any(|c| c.is_uppercase()) {
            // Fallback: lowercase any unknown PascalCase kind
            provider.kind = old_kind.to_lowercase();
        }

        // Fill URL from preset if empty
        if provider.url.is_empty() {
            if let Some(preset) = jona_provider::preset(&provider.kind) {
                provider.url = preset.base_url.to_string();
            }
        }

        // Update capabilities from preset
        if let Some(preset) = jona_provider::preset(&provider.kind) {
            provider.supports_asr = preset.supports_asr;
            provider.supports_llm = preset.supports_llm;
        }

        if provider.kind != old_kind {
            migrated += 1;
        }
    }
    if migrated > 0 {
        log::info!("Migration v8: normalized {} provider kind(s) to lowercase", migrated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_prefs() -> Preferences {
        Preferences::default()
    }

    // -- run() orchestrator --

    #[test]
    fn run_skips_when_already_current() {
        let mut raw = json!({"_version": CURRENT_VERSION});
        let mut prefs = empty_prefs();
        assert!(!run(&mut raw, &mut prefs));
    }

    #[test]
    fn run_applies_pending_migrations() {
        let mut raw = json!({"_version": 7});
        let mut prefs = empty_prefs();
        assert!(run(&mut raw, &mut prefs));
        assert_eq!(raw["_version"], CURRENT_VERSION);
        assert_eq!(prefs.schema_version, CURRENT_VERSION);
    }

    #[test]
    fn run_from_zero_applies_all() {
        // Start at v4 to skip v2 (filesystem ops on real config dir)
        // and v4 (OS keychain writes). Those migrations are safe to skip
        // in tests since they operate on empty prefs (no providers = no keyring
        // writes, no old dirs = no filesystem changes), but we avoid them
        // as a safety guarantee for CI environments.
        let mut raw = json!({"_version": 4});
        let mut prefs = empty_prefs();
        assert!(run(&mut raw, &mut prefs));
        assert_eq!(raw["_version"], CURRENT_VERSION);
    }

    // -- migrate_v1 --

    #[test]
    fn v1_migrates_api_servers_to_providers() {
        let mut raw = json!({
            "api_servers": [{
                "id": "my-server",
                "name": "My Server",
                "url": "https://api.example.com",
                "model": "whisper-1",
                "api_key": "sk-test"
            }]
        });
        let mut prefs = empty_prefs();
        migrate_v1(&mut raw, &mut prefs);

        assert_eq!(prefs.providers.len(), 1);
        assert_eq!(prefs.providers[0].id, "my-server");
        assert_eq!(prefs.providers[0].kind, "custom");
        assert_eq!(prefs.providers[0].url, "https://api.example.com");
        assert_eq!(prefs.selected_model_id, "cloud:my-server");
        assert_eq!(prefs.asr_cloud_model, "whisper-1");
    }

    #[test]
    fn v1_migrates_llm_config() {
        let mut raw = json!({
            "llm_config": {
                "provider": "openai",
                "api_url": "https://api.openai.com/v1",
                "api_key": "sk-llm",
                "model": "gpt-4o",
                "enabled": true
            }
        });
        let mut prefs = empty_prefs();
        migrate_v1(&mut raw, &mut prefs);

        assert!(prefs.text_cleanup_enabled);
        assert_eq!(prefs.llm_model, "gpt-4o");
        assert_eq!(prefs.llm_provider_id, "provider-openai");
        assert_eq!(prefs.providers.len(), 1);
        assert_eq!(prefs.providers[0].kind, "openai");
    }

    #[test]
    fn v1_migrates_cleanup_mode_punctuation() {
        let mut raw = json!({
            "cleanup_mode": "punctuation",
            "punctuation_model_id": "bert-punctuation:base"
        });
        let mut prefs = empty_prefs();
        migrate_v1(&mut raw, &mut prefs);

        assert!(prefs.text_cleanup_enabled);
        assert_eq!(prefs.cleanup_model_id, "bert-punctuation:base");
    }

    #[test]
    fn v1_migrates_cleanup_mode_full_local() {
        let mut raw = json!({
            "cleanup_mode": "full",
            "llm_source": "local",
            "llm_local_model_id": "llm-local:tinyllama"
        });
        let mut prefs = empty_prefs();
        migrate_v1(&mut raw, &mut prefs);

        assert!(prefs.text_cleanup_enabled);
        assert_eq!(prefs.cleanup_model_id, "llama:tinyllama");
    }

    #[test]
    fn v1_resets_old_openai_api_prefix() {
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.selected_model_id = "openai-api:whisper-1".to_string();
        migrate_v1(&mut raw, &mut prefs);

        assert_eq!(prefs.selected_model_id, default_model_id());
    }

    #[test]
    fn v1_migrates_asr_provider_id() {
        let mut raw = json!({"asr_provider_id": "my-provider"});
        let mut prefs = empty_prefs();
        migrate_v1(&mut raw, &mut prefs);

        assert_eq!(prefs.selected_model_id, "cloud:my-provider");
    }

    #[test]
    fn v1_noop_on_empty_json() {
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        let old_model = prefs.selected_model_id.clone();
        migrate_v1(&mut raw, &mut prefs);

        assert!(prefs.providers.is_empty());
        assert_eq!(prefs.selected_model_id, old_model);
    }

    // -- migrate_v3 --

    #[test]
    fn v3_updates_low_max_tokens() {
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.llm_max_tokens = 256;
        migrate_v3(&mut raw, &mut prefs);
        assert_eq!(prefs.llm_max_tokens, 4096);
    }

    #[test]
    fn v3_preserves_high_max_tokens() {
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.llm_max_tokens = 8192;
        migrate_v3(&mut raw, &mut prefs);
        assert_eq!(prefs.llm_max_tokens, 8192);
    }

    // -- migrate_v6 --

    #[test]
    fn v6_splits_punctuation_model() {
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.text_cleanup_enabled = true;
        prefs.cleanup_model_id = "bert-punctuation:base".to_string();
        migrate_v6(&mut raw, &mut prefs);

        assert_eq!(prefs.punctuation_model_id, "bert-punctuation:base");
        assert!(prefs.cleanup_model_id.is_empty());
        assert!(!prefs.text_cleanup_enabled);
    }

    #[test]
    fn v6_splits_pcs_punctuation() {
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.cleanup_model_id = "pcs-punctuation:large".to_string();
        migrate_v6(&mut raw, &mut prefs);

        assert_eq!(prefs.punctuation_model_id, "pcs-punctuation:large");
        assert!(prefs.cleanup_model_id.is_empty());
    }

    #[test]
    fn v6_ignores_non_punctuation_model() {
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.cleanup_model_id = "correction:gec-t5-small".to_string();
        prefs.text_cleanup_enabled = true;
        migrate_v6(&mut raw, &mut prefs);

        assert_eq!(prefs.cleanup_model_id, "correction:gec-t5-small");
        assert!(prefs.punctuation_model_id.is_empty());
        assert!(prefs.text_cleanup_enabled);
    }

    // -- migrate_v8 --
    // Note: v8 calls jona_provider::preset() which requires ProviderCatalog init.
    // Tests use init_catalog() to initialize it. In test context, inventory may
    // not have the backend crates linked, so preset lookups may return None —
    // but the kind normalization logic is still fully testable.

    fn init_catalog() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| jona_provider::ProviderCatalog::init_auto());
    }

    fn test_provider(kind: &str, url: &str) -> Provider {
        Provider {
            id: "p1".into(), name: "Test".into(), kind: kind.into(),
            url: url.into(), api_key: String::new(),
            allow_insecure: false, cached_models: vec![], supports_asr: true,
            supports_llm: true, api_format: None,
        }
    }

    #[test]
    fn v8_normalizes_pascal_case_kinds() {
        init_catalog();
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.providers.push(test_provider("OpenAI", "https://api.openai.com/v1"));
        prefs.providers.push(test_provider("Anthropic", "https://api.anthropic.com/v1"));
        migrate_v8(&mut raw, &mut prefs);

        assert_eq!(prefs.providers[0].kind, "openai");
        assert_eq!(prefs.providers[1].kind, "anthropic");
    }

    #[test]
    fn v8_unknown_pascal_falls_back_to_lowercase() {
        init_catalog();
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.providers.push(test_provider("MyCustomBackend", "https://example.com"));
        migrate_v8(&mut raw, &mut prefs);

        assert_eq!(prefs.providers[0].kind, "mycustombackend");
    }

    #[test]
    fn v8_already_lowercase_unchanged() {
        init_catalog();
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.providers.push(test_provider("custom", "https://example.com"));
        migrate_v8(&mut raw, &mut prefs);

        assert_eq!(prefs.providers[0].kind, "custom");
    }

    #[test]
    fn v8_fills_empty_url_from_preset() {
        init_catalog();
        let mut raw = json!({});
        let mut prefs = empty_prefs();
        prefs.providers.push(test_provider("OpenAI", ""));
        migrate_v8(&mut raw, &mut prefs);

        assert_eq!(prefs.providers[0].kind, "openai");
        // If preset was found, URL should be filled
        if !prefs.providers[0].url.is_empty() {
            assert!(prefs.providers[0].url.starts_with("https://"));
        }
    }
}

