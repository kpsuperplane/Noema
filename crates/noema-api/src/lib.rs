//! Transport-neutral GraphQL API for Noema clients.
//!
//! Application composition lives in `noema-host`; this crate consumes the
//! resulting service handles and exposes the shared schema used by every shell.

/// GraphQL client API facade.
pub mod graphql;
#[cfg(test)]
mod test_support;

pub use graphql::RequestPrincipal;
