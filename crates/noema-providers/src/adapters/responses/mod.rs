//! Shared Responses API request lowering, parsing, and transport.

mod input;
mod output;
mod request;
mod sse;
mod tools;
mod transport;

pub use crate::response_support::http::normalize_base_url;
pub use input::*;
pub use output::{ResponsesResponse, ResponsesUsage};
pub use request::ResponsesRequest;
pub(crate) use request::{CODEX_RESPONSES_PROFILE, OPENAI_RESPONSES_PROFILE};
pub(crate) use tools::ResponsesToolNameMap;
pub use tools::{ResponsesTool, ResponsesToolChoice};
pub use transport::{ResponsesTransport, header_value};
pub(crate) use transport::{ResponsesWebSocketError, ResponsesWebSocketSession};

pub use crate::response_support::StructuredResponseDiagnosticContext as ResponsesDiagnosticContext;

#[cfg(test)]
mod tests;
