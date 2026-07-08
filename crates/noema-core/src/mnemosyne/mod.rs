//! Mnemosyne local service client and lifecycle support.

mod client;
mod endpoint;
mod lifecycle;

#[cfg(test)]
mod tests;

pub use client::{
    MnemosyneAddMemoryRequest, MnemosyneClient, MnemosyneClientError, MnemosyneListMemoriesRequest,
    MnemosyneListMemoriesResponse, MnemosyneMemory, MnemosyneMessage, MnemosyneSearchRequest,
    MnemosyneSearchResponse,
};
pub use endpoint::{MnemosyneConnection, allocate_loopback_port};
pub use lifecycle::{
    MnemosyneLifecycle, MnemosyneLifecycleError, NOEMA_MNEMOSYNE_SIDECAR_COMMAND_ENV,
};
