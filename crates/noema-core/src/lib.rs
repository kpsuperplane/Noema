//! Transitional Noema application composition and GraphQL API.
//!
//! Governed execution lives in `noema-runtime`; this crate temporarily retains
//! configuration, host composition, and the GraphQL surface during decomposition.

#[cfg(test)]
mod artifact_store_composition_tests;
/// Configuration loading and provider selection.
pub mod config;
/// GraphQL client API facade.
pub mod graphql;
mod mcp_completion;
/// Onboarding status derived from provider account readiness.
pub mod onboarding;
/// Shared runtime host for daemon and desktop client surfaces.
pub mod runtime_host;
#[cfg(test)]
mod test_support;

pub use config::{
    Config, ConfigError, ConfigOverrides, DEFAULT_NOEMA_CONFIG_YAML, DaemonResolvedConfig,
    ResolvedConfig, WebConfig,
};
pub use graphql::RequestPrincipal;
pub use noema_capabilities_mcp::{McpCalibrationStatus, McpTransportKind, McpTrustClassification};
pub use onboarding::{
    OnboardingStatus, OnboardingStep, OnboardingStepStatus, onboarding_status_from_account,
    onboarding_status_from_options,
};
pub use runtime_host::{NoemaRuntimeHost, RuntimeHostError};
