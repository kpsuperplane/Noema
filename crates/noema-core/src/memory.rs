//! Memory subsystem types, extraction, consolidation, and graph retrieval
//! policy.

/// Future-write memory consolidation types and prompts.
pub mod consolidation;
/// Shared memory/domain error types.
pub mod error;
/// Pure ordinary-chat memory extraction proposal layer.
pub mod extraction;
mod model;
/// Neutral memory classification types.
pub mod types;

pub use model::*;
