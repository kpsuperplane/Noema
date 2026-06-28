//! Embedded SurrealDB-backed canonical Noema store.

mod error;
mod runtime;
mod schema;

#[cfg(test)]
mod tests;

pub use error::StoreError;
pub use runtime::{NoemaStore, StoreConfig};
