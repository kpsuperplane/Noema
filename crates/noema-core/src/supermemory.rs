//! Supermemory local service client and lifecycle support.

mod binary;
mod client;
mod config;
mod endpoint;
mod lifecycle;
mod model_proxy;

#[cfg(test)]
mod tests;

pub use binary::{
    NOEMA_SUPERMEMORY_SERVER_ENV, SupermemoryBinaryError, SupermemoryBinaryResolver,
    SupermemoryBinarySource, SupermemoryServerBinary,
};
pub use client::{
    SupermemoryClient, SupermemoryClientError, SupermemoryConversationIngestRequest,
    SupermemoryConversationMessage, SupermemorySearchRequest, SupermemorySearchResponse,
    SupermemorySearchResult,
};
pub use config::SupermemoryRuntimeConfig;
pub use endpoint::{SupermemoryConnection, allocate_loopback_port};
pub use lifecycle::{SupermemoryLifecycle, SupermemoryLifecycleError};
pub use model_proxy::{
    SupermemoryModelProxy, SupermemoryModelProxyConfig, SupermemoryModelProxyError,
};
