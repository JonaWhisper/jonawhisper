use serde_json::Value;
use crate::state::{config_dir, default_model_id, Provider, ProviderKind, Preferences};

/// Current schema version. Bump when adding a new migration.
const CURRENT_VERSION: u32 = 4;

type MigrationFn = fn(&mut Value, &mut Preferences);

const MIGRATIONS: &[(u32, &str, MigrationFn)] = &[
    (1, "Unify providers and cleanup settings", migrate_v1),
    (2, "Centralize model storage", migrate_v2),
    (3, "Update llm_max_tokens default to 4096", migrate_v3),
    (4, "Migrate API keys to OS keychain", migrate_v4),
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
fn migrate_v1(raw: &mut Value, prefs: &mut Preferences) {
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
                        kind: ProviderKind::Custom,
                        url: url.to_string(),
                        api_key: api_key.to_string(),
                        allow_insecure: false,
                        cached_models: Vec::new(),
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
                    let kind = if provider_str == "anthropic" {
                        ProviderKind::Anthropic
                    } else {
                        ProviderKind::OpenAI
                    };
                    let id = format!("provider-{}", provider_str);
                    prefs.providers.push(Provider {
                        id: id.clone(),
                        name: kind.display_name().to_string(),
                        kind,
                        url: api_url.to_string(),
                        api_key: api_key.to_string(),
                        allow_insecure: false,
                        cached_models: Vec::new(),
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
    if prefs.text_cleanup_enabled && (prefs.cleanup_model_id.is_empty() || prefs.cleanup_model_id == "cloud") {
        if !prefs.llm_provider_id.is_empty() {
            prefs.cleanup_model_id = format!("cloud:{}", prefs.llm_provider_id);
        }
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
        if std::fs::read_dir(&old_dir).map_or(false, |mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&old_dir);
            log::info!("Migration v2: removed empty dir {}", old_dir.display());
        }
    }

    // Clean up ~/.local/share/whisper-dictate/ if empty
    let wd_dir = std::path::PathBuf::from(shellexpand::tilde("~/.local/share/whisper-dictate").as_ref());
    if wd_dir.exists() {
        if std::fs::read_dir(&wd_dir).map_or(false, |mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&wd_dir);
            log::info!("Migration v2: removed empty dir {}", wd_dir.display());
        }
    }
}

/// v3: Update llm_max_tokens from old default (256) to new default (4096)
fn migrate_v3(_raw: &mut Value, prefs: &mut Preferences) {
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

