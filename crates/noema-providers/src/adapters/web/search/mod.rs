//! Concrete web search backends.

mod duckduckgo;
mod exa;

pub use duckduckgo::{
    DuckDuckGoSearchBackend, default_runtime_provider as default_web_search_backend,
};
pub use exa::{EXA_SEARCH_CONTRACT, EXA_SEARCH_PROVIDER_ID, ExaSearchClient};
