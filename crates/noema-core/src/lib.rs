//! Core Noema types and runtimes.
//!
//! This crate contains configuration loading, provider adapters, daemon
//! runtime support and the memory retrieval model. Filesystem layout and
//! developer diagnostics live in `noema-home`.

// Resolve legacy GraphQL/protocol documentation links without forwarding the
// task type as part of noema-core's public API.
#[allow(unused_imports)]
use noema_tasks::TaskStatus;

/// Shared execution roles and role-aware tool dispatch policy.
pub mod agent_execution;
#[cfg(test)]
mod artifact_store_composition_tests;
/// Configuration loading and provider selection.
pub mod config;
/// Local daemon runtime and web protocol types.
pub mod daemon;
/// GraphQL client API facade.
pub mod graphql;
mod mcp_completion;
/// Onboarding status derived from provider account readiness.
pub mod onboarding;
/// Shared runtime host for daemon and desktop client surfaces.
pub mod runtime_host;
/// First-party governed web search capability.
pub mod search;
#[cfg(test)]
mod test_support;
#[doc(hidden)]
pub mod web_fetch;

pub use config::{
    Config, ConfigError, ConfigOverrides, DEFAULT_NOEMA_CONFIG_YAML, DaemonResolvedConfig,
    ResolvedConfig, WebConfig,
};
pub use daemon::{
    AgentStatus, DaemonError, StartedConversation, TurnActivityStatus, TurnTranscriptItem,
};
pub use graphql::RequestPrincipal;
pub use noema_capabilities_mcp::{McpCalibrationStatus, McpTransportKind, McpTrustClassification};
pub use onboarding::{
    OnboardingStatus, OnboardingStep, OnboardingStepStatus, onboarding_status_from_account,
    onboarding_status_from_options,
};
pub use runtime_host::{NoemaRuntimeHost, RuntimeHostError};
