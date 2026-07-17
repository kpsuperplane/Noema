//! First-party local GGUF provider implementation.

mod catalog;
#[cfg(test)]
mod catalog_tests;
mod download;
mod download_support;
#[cfg(test)]
mod download_tests;
#[cfg(feature = "local-model-evals")]
mod eval;
mod hardware;
mod manager;
mod provider;
mod runtime;
mod runtime_assets;

pub use catalog::LocalModelCatalog;
#[cfg(test)]
pub use catalog::LocalModelCatalogError;
pub use download::{LocalModelInstallError, LocalModelInstaller};
#[cfg(feature = "local-model-evals")]
pub use eval::{
    LocalModelEvalError, LocalModelEvalRuntimeVersion, LocalModelEvalSession,
    LocalModelEvalSessionConfig, MaterializeVerifiedEvalModelRequest, VerifiedEvalModelSource,
    local_model_eval_runtime_version, materialize_verified_eval_model,
};
pub use hardware::detect_local_hardware_profiles;
pub use provider::LocalModelsProvider;
pub use runtime::{
    LlamaServerCandidate, LlamaServerConfig, LlamaServerError, LlamaServerSupervisor,
};
pub use runtime_assets::bundled_llama_server_candidates_in;
#[cfg(feature = "local-model-evals")]
pub use runtime_assets::{LLAMA_CPP_COMMIT, LLAMA_CPP_RELEASE_TAG};
