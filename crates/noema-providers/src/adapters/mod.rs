//! Concrete provider adapters and provider-owned integration services.

mod account_files;
mod account_service;
pub(crate) mod auth;
pub(crate) mod codex;
pub(crate) mod foundation;
mod hosted;
pub(crate) mod openai;
pub(crate) mod openrouter;
pub(crate) mod responses;
pub(crate) mod web;

pub(crate) use account_files::SecretInputStore;
pub use account_service::{
    ProviderAccountService, ProviderCredential, ProviderCredentialAccess,
    ProviderCredentialAccessHandle, ProviderCredentialFuture,
};
pub use foundation::FoundationLocalProvider;
pub use hosted::{ProviderBootstrap, hosted_provider_from_config, provider_bootstrap_from_config};
pub use web::{
    EXA_FETCH_PROVIDER_ID, EXA_SEARCH_PROVIDER_ID, ExaFetchClient, ExaSearchClient,
    default_web_fetch_backend, default_web_search_backend, summarize_markdown,
    web_fetch_summarizer_prompt,
};

pub(crate) use crate::reqwest_transport_error;

#[cfg(test)]
pub(crate) mod test_support;
