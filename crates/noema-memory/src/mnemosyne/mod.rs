//! Mnemosyne local service client, operations adapter, and lifecycle support.

mod client;
mod endpoint;
mod lifecycle;
mod service;

#[cfg(test)]
mod tests;

pub use client::{MnemosyneClient, MnemosyneClientError};
pub use endpoint::{MnemosyneConnection, allocate_loopback_port};
pub use lifecycle::{
    MnemosyneLifecycle, MnemosyneLifecycleError, NOEMA_MNEMOSYNE_SIDECAR_COMMAND_ENV,
};
pub use service::{MnemosyneMemoryService, MnemosyneMemoryServiceAccess};
