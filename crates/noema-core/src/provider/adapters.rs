//! Concrete provider adapters and transport helpers.

/// Codex OAuth token storage and device-code authentication.
pub mod codex_oauth;
/// Provider adapter for Codex direct Responses API.
pub mod codex_responses;
/// Apple Foundation Models bridge process lifecycle.
pub mod foundation_bridge_process;
/// Apple Foundation Models bridge protocol.
pub mod foundation_bridge_protocol;
/// Apple Foundation Models local provider adapter.
pub mod foundation_local;
/// First-party local GGUF provider adapter.
pub mod local_models;
/// Parser for structured Noema JSON response deltas.
pub(crate) mod noema_response_stream;
/// Provider adapter for the OpenAI Responses API.
pub mod openai;
/// Shared transport and parser for Responses API providers.
pub mod responses;
mod responses_format;
mod responses_input;
mod responses_output;
mod responses_tools;
mod transport_error;
pub(crate) use transport_error::reqwest_transport_error;
/// Shared Server-Sent Events parser for Responses API streams.
pub(crate) mod sse;
/// Shared one-shot HTTP server helpers for provider and integration tests.
#[cfg(test)]
pub(crate) mod test_support;
