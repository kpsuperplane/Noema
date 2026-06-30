//! Memory subsystem types, extraction, consolidation, and retrieval policy.
//!
//! The store mirrors canonical memory rows and policy rules so retrieval
//! behavior can be developed and tested independently from persistence.

/// Future-write memory consolidation types and prompts.
pub mod consolidation;
/// Shared memory/domain error types.
pub mod error;
/// Pure ordinary-chat memory extraction proposal layer.
pub mod extraction;
mod model;
mod store;
mod store_helpers;
#[cfg(test)]
mod tests;
/// Neutral memory classification and candidate types.
pub mod types;

pub use model::*;
pub use store::MemoryStore;
