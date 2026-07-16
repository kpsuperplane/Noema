//! Concrete web search backends.

mod duckduckgo;
mod exa;

pub use duckduckgo::default_runtime_provider as default_web_search_backend;
pub use exa::{EXA_SEARCH_PROVIDER_ID, ExaSearchClient};
