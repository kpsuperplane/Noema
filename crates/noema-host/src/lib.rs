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

pub use config::{BrowserConfig, DEFAULT_NOEMA_CONFIG_YAML, HostConfig, WebConfig};
#[cfg(feature = "composition")]
pub use config::{Config, ConfigError};
pub use onboarding::{
    OnboardingService, OnboardingServiceError, OnboardingStatus, OnboardingStep,
    OnboardingStepStatus,
};
pub use runtime_host::{
    ArtifactDiagnosticHandle, ArtifactDiagnosticOperations, HostServices, NoemaHost,
    RuntimeHostError,
};

/// Run a private browser worker when the process has worker arguments.
///
/// The application entrypoint must exit with the returned status.
#[cfg(feature = "composition")]
#[must_use]
pub fn run_browser_worker_if_requested() -> Option<i32> {
    noema_providers::run_browser_worker_if_requested()
}
#[cfg(feature = "composition")]
pub use runtime_host::{
    start_from_loaded_config, start_from_process_env,
    start_from_process_env_with_local_model_runtime_root,
};
