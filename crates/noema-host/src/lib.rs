//! Noema application composition and lifecycle ownership.
//!
//! This crate owns configuration, startup, service assembly, onboarding, and
//! dependency-ordered shutdown. Runtime and API behavior remain in their
//! dedicated crates.

#[cfg(all(test, feature = "composition"))]
mod artifact_store_composition_tests;
#[cfg(feature = "composition")]
mod composition;
/// Host-owned configuration loading and resolved startup input.
pub mod config;
#[cfg(feature = "composition")]
mod mcp_completion;
/// Cross-subsystem first-run onboarding operations.
pub mod onboarding;
/// Assembled application host and lifecycle.
pub mod runtime_host;

#[cfg(feature = "composition")]
pub use config::{Config, ConfigError, ConfigOverrides};
pub use config::{DEFAULT_NOEMA_CONFIG_YAML, HostConfig, WebConfig};
pub use onboarding::{
    OnboardingService, OnboardingServiceError, OnboardingStatus, OnboardingStep,
    OnboardingStepStatus, onboarding_status_from_account, onboarding_status_from_options,
};
pub use runtime_host::{
    ArtifactDiagnosticHandle, ArtifactDiagnosticOperations, HostServices, NoemaHost,
    RuntimeHostError,
};
#[cfg(feature = "composition")]
pub use runtime_host::{
    start_from_loaded_config, start_from_process_env,
    start_from_process_env_with_local_model_runtime_root,
};
