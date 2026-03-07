pub mod app;
pub mod audio;
pub mod engines;
pub mod history;
pub mod permissions;
pub mod providers;
pub mod settings;

use jona_engines::EngineCatalog;

/// Access the global engine catalog singleton.
pub(crate) fn catalog() -> &'static EngineCatalog {
    EngineCatalog::global()
}
