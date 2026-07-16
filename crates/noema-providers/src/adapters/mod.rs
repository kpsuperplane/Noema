//! Concrete provider adapters and provider-owned integration services.

mod account_files;
pub mod auth;
pub mod codex;
pub mod foundation;
pub mod openai;
pub(crate) mod responses;
mod transport_error;
pub mod web;

pub use account_files::SecretInputStore;
pub use auth::{ProviderAuthManager, ensure_provider_account_home};
pub use codex::{
    CodexOAuthClient, CodexResponsesProvider, CodexTokenStore, refresh_provider_model_profiles,
};
pub use foundation::{
    FOUNDATION_LOCAL_COMPACT_SUMMARY_TARGET_TOKENS, FOUNDATION_LOCAL_CONTEXT_WINDOW_TOKENS,
    FOUNDATION_LOCAL_DEFAULT_OUTPUT_RESERVE_TOKENS, FOUNDATION_LOCAL_PROVIDER,
    FoundationBridgeError, FoundationLocalProvider,
};
pub use openai::OpenAiProvider;
pub use web::{
    DirectHttpClient, DuckDuckGoSearchBackend, EXA_EXTRACTION, EXA_FETCH_PROVIDER_ID,
    EXA_SEARCH_CONTRACT, EXA_SEARCH_PROVIDER_ID, ExaFetchClient, ExaSearchClient,
    default_web_fetch_backend, default_web_search_backend, summarize_markdown,
    web_fetch_summarizer_prompt,
};

pub(crate) use transport_error::reqwest_transport_error;

#[cfg(test)]
pub(crate) mod test_support;
