//! Provider catalog — orchestrates cloud provider backends registered via inventory.

use jona_types::{CloudProvider, ProviderKind, ProviderRegistration};
use std::collections::HashMap;
use std::sync::OnceLock;

// Re-export for call site compatibility
pub use jona_types::ProviderError;

static CATALOG: OnceLock<ProviderCatalog> = OnceLock::new();

pub struct ProviderCatalog {
    backends: HashMap<ProviderKind, Box<dyn CloudProvider>>,
}

// Safety: CloudProvider is Send + Sync, HashMap is Send + Sync when K/V are.
unsafe impl Send for ProviderCatalog {}
unsafe impl Sync for ProviderCatalog {}

impl ProviderCatalog {
    /// Initialize the catalog from all inventory-registered providers.
    /// Must be called once at startup.
    pub fn init_auto() {
        let mut backends = HashMap::new();

        for reg in inventory::iter::<ProviderRegistration> {
            for &kind in reg.kinds {
                let backend = (reg.factory)();
                log::debug!("ProviderCatalog: registered {:?}", kind);
                backends.insert(kind, backend);
            }
        }

        log::info!("ProviderCatalog: {} provider kinds registered", backends.len());
        CATALOG.set(ProviderCatalog { backends }).ok();
    }

    fn global() -> &'static ProviderCatalog {
        CATALOG.get().expect("ProviderCatalog not initialized — call init_auto() first")
    }
}

/// Get the appropriate cloud provider backend for a given kind.
pub fn backend(kind: ProviderKind) -> &'static dyn CloudProvider {
    let catalog = ProviderCatalog::global();
    catalog
        .backends
        .get(&kind)
        .map(|b| &**b)
        .unwrap_or_else(|| {
            panic!("No provider backend registered for {:?}", kind);
        })
}
