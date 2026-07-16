//! Shared Responses API request lowering, parsing, and transport.

mod input;
mod output;
mod request;
mod sse;
mod tools;
mod transport;

pub use input::*;
pub use output::{ResponsesResponse, ResponsesUsage};
pub use request::ResponsesRequest;
pub(crate) use request::{CODEX_RESPONSES_PROFILE, OPENAI_RESPONSES_PROFILE};
pub use tools::{ResponsesTool, ResponsesToolChoice};
pub use transport::{ResponsesTransport, header_value, normalize_base_url};

pub use crate::response_support::StructuredResponseDiagnosticContext as ResponsesDiagnosticContext;

#[cfg(test)]
mod tests;
