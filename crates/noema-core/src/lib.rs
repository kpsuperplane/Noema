//! Transitional Noema GraphQL API.
//!
//! Application composition lives in `noema-host` and governed execution lives
//! in `noema-runtime`. This crate temporarily retains only the GraphQL surface
//! during decomposition.

/// GraphQL client API facade.
pub mod graphql;
#[cfg(test)]
mod test_support;

pub use graphql::RequestPrincipal;
pub use noema_capabilities_mcp::{McpCalibrationStatus, McpTransportKind, McpTrustClassification};
