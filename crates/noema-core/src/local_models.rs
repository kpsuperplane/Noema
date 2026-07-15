//! Bundled local-model catalog and generic recommendation logic.

mod catalog;
#[cfg(test)]
mod catalog_tests;

pub use catalog::{
    LocalHardwareProfile, LocalModelBackend, LocalModelBuild, LocalModelCatalog,
    LocalModelCatalogEntry, LocalModelCatalogError, LocalModelRecommendation,
};
