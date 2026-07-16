//! Bundled local-model catalog and generic recommendation logic.

mod catalog;
#[cfg(test)]
mod catalog_tests;
mod download;
mod download_support;
#[cfg(test)]
mod download_tests;
mod hardware;
mod provider;
mod runtime;
mod runtime_assets;

pub use catalog::{
    LocalHardwareProfile, LocalModelBuild, LocalModelCatalog, LocalModelCatalogEntry,
    LocalModelCatalogError, LocalModelRecommendation,
};
pub use download::{
    HuggingFaceLocalModelImport, LocalFileModelImport, LocalModelInstallError, LocalModelInstaller,
};
pub use hardware::{LocalHardwareProbeError, detect_local_hardware_profiles};
pub use provider::LocalModelsProvider;
pub use runtime::{
    LlamaServerCandidate, LlamaServerConfig, LlamaServerEndpoint, LlamaServerError,
    LlamaServerSupervisor, LocalModelRuntimeStatus,
};
pub use runtime_assets::{
    LLAMA_CPP_COMMIT, LLAMA_CPP_RELEASE_TAG, LLAMA_CPP_RUNTIME_ASSETS,
    LLAMA_SERVER_SIDECAR_BASENAME, LlamaCppRuntimeAsset, LlamaCppRuntimeAssetRole,
    NOEMA_LLAMA_SERVER_PATH_ENV, bundled_llama_server_candidates,
    bundled_llama_server_candidates_in, tauri_sidecar_input_name,
};
