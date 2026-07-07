//! Supermemory local service client and lifecycle support.

mod client;
mod config;
mod lifecycle;

#[cfg(test)]
mod tests;

pub use client::{
    SupermemoryClient, SupermemoryClientError, SupermemoryConversationIngestRequest,
    SupermemorySearchRequest, SupermemorySearchResponse, SupermemorySearchResult,
};
pub use config::SupermemoryRuntimeConfig;
pub use lifecycle::{SupermemoryLifecycle, SupermemoryLifecycleError};
