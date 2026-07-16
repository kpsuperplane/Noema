//! Concrete web fetch backends.

mod direct_http;
mod exa;
mod extraction;
mod summarize;
mod url_policy;

pub use direct_http::default_runtime_provider as default_web_fetch_backend;
pub use exa::{EXA_FETCH_PROVIDER_ID, ExaFetchClient};
pub use summarize::{summarize_markdown, summarizer_prompt as web_fetch_summarizer_prompt};
