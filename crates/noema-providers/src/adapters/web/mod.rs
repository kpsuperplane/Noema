//! Concrete web search and fetch provider backends.

mod exa_transport;

pub mod fetch;
pub mod search;

pub use fetch::{
    DirectHttpClient, EXA_EXTRACTION, EXA_FETCH_PROVIDER_ID, ExaFetchClient,
    default_web_fetch_backend, summarize_markdown, web_fetch_summarizer_prompt,
};
pub use search::{
    DuckDuckGoSearchBackend, EXA_SEARCH_CONTRACT, EXA_SEARCH_PROVIDER_ID, ExaSearchClient,
    default_web_search_backend,
};
