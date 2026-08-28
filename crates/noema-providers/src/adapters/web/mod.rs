//! Concrete web search and fetch provider backends.

mod exa_transport;
mod firecrawl;
mod normalize;
mod tinyfish;

pub mod browse;
pub mod fetch;
pub mod search;

pub use browse::default_web_browse_backend;
pub(crate) use browse::run_worker_if_requested;
pub use exa_transport::ExaWebClient;
pub use fetch::{
    EXA_FETCH_PROVIDER_ID, default_web_fetch_backend, summarize_markdown,
    web_fetch_summarizer_prompt,
};
pub use firecrawl::{FIRECRAWL_PROVIDER_ID, FirecrawlWebClient};
pub use search::{EXA_SEARCH_PROVIDER_ID, default_web_search_backend};
pub use tinyfish::{TINYFISH_PROVIDER_ID, TinyFishWebClient};
