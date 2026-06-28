//! Embedded SurrealDB-backed canonical Noema store.

mod conversations;
mod error;
mod ids;
pub mod objects;
mod provider_accounts;
mod runtime;
mod schema;

#[cfg(test)]
mod tests;

pub use error::StoreError;
pub use runtime::{NoemaStore, StoreConfig};
