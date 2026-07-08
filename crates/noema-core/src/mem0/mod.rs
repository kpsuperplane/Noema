//! Mem0 local service client and lifecycle support.

mod client;
mod endpoint;
mod lifecycle;

#[cfg(test)]
mod tests;

pub use client::{
    Mem0AddMemoryRequest, Mem0Client, Mem0ClientError, Mem0ListMemoriesRequest,
    Mem0ListMemoriesResponse, Mem0Memory, Mem0Message, Mem0SearchRequest, Mem0SearchResponse,
};
pub use endpoint::{Mem0Connection, allocate_loopback_port};
pub use lifecycle::{Mem0Lifecycle, Mem0LifecycleError, NOEMA_MEM0_SIDECAR_COMMAND_ENV};
