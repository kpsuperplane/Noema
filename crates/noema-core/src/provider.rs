//! Concrete provider adapters and runtime support.
//!
//! Provider-neutral contracts and configuration live in `noema-providers`.

/// Concrete model provider adapters and transport helpers.
pub mod adapters;
/// Provider authentication support.
pub mod auth;
/// Provider model/profile catalog refresh helpers.
pub mod model_catalog;
/// Write-only secret-input provider account storage.
pub mod secret_input;
