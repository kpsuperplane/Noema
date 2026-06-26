//! Memory storage and deterministic retrieval policy model.
//!
//! The store mirrors canonical memory rows and policy rules so retrieval
//! behavior can be developed and tested independently from persistence.

mod model;
mod store;
mod store_helpers;
#[cfg(test)]
mod tests;

pub use model::*;
pub use store::MemoryStore;
