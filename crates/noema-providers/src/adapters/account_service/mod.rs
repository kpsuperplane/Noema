//! Provider-owned account orchestration and credential access.

mod credentials;
pub(super) mod filesystem;
pub(super) mod gates;
mod service;

pub use credentials::{
    ProviderCredential, ProviderCredentialAccess, ProviderCredentialAccessHandle,
    ProviderCredentialFuture,
};
pub use service::ProviderAccountService;
