//! Provider catalog — orchestrates cloud provider backends registered via inventory.

use jona_types::{CloudProvider, Provider, ProviderPreset, ProviderRegistration};
use std::collections::HashMap;
use std::sync::OnceLock;

// Re-export for call site compatibility
pub use jona_types::ProviderError;

static CATALOG: OnceLock<ProviderCatalog> = OnceLock::new();

pub struct ProviderCatalog {
    backends: HashMap<&'static str, Box<dyn CloudProvider>>,
    presets: Vec<&'static ProviderPreset>,
    preset_map: HashMap<&'static str, &'static ProviderPreset>,
}

// Safety: CloudProvider is Send + Sync, HashMap is Send + Sync when K/V are.
unsafe impl Send for ProviderCatalog {}
unsafe impl Sync for ProviderCatalog {}

impl ProviderCatalog {
    /// Initialize the catalog from all inventory-registered providers and presets.
    /// Must be called once at startup.
    pub fn init_auto() {
        let mut backends = HashMap::new();

        for reg in inventory::iter::<ProviderRegistration> {
            let backend = (reg.factory)();
            log::debug!("ProviderCatalog: registered backend {}", reg.backend_id);
            backends.insert(reg.backend_id, backend);
        }

        let mut presets: Vec<&'static ProviderPreset> =
            inventory::iter::<ProviderPreset>.into_iter().collect();
        presets.sort_by_key(|p| p.id);

        let preset_map: HashMap<&'static str, &'static ProviderPreset> =
            presets.iter().map(|p| (p.id, *p)).collect();

        log::info!(
            "ProviderCatalog: {} backends, {} presets registered",
            backends.len(),
            presets.len()
        );
        CATALOG
            .set(ProviderCatalog {
                backends,
                presets,
                preset_map,
            })
            .ok();
    }

    fn global() -> &'static ProviderCatalog {
        CATALOG
            .get()
            .expect("ProviderCatalog not initialized — call init_auto() first")
    }
}

/// Get the cloud provider backend for a given backend ID.
pub fn backend(id: &str) -> &'static dyn CloudProvider {
    let catalog = ProviderCatalog::global();
    catalog
        .backends
        .get(id)
        .map(|b| &**b)
        .unwrap_or_else(|| {
            panic!("No provider backend registered for {}", id);
        })
}

/// Get the cloud provider backend for a given provider (resolves format from preset or provider).
pub fn backend_for_provider(provider: &Provider) -> &'static dyn CloudProvider {
    let id = preset(&provider.kind)
        .map(|p| p.backend_id)
        .unwrap_or_else(|| provider.resolved_api_format());
    backend(id)
}

/// Get all registered presets (sorted by ID).
pub fn presets() -> &'static [&'static ProviderPreset] {
    &ProviderCatalog::global().presets
}

/// Look up a preset by ID.
pub fn preset(id: &str) -> Option<&'static ProviderPreset> {
    ProviderCatalog::global().preset_map.get(id).copied()
}
