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
/// Parser for structured Noema JSON response deltas.
pub(crate) mod noema_response_stream;
/// Provider adapter for the OpenAI Responses API.
pub mod openai;
/// Shared transport and parser for Responses API providers.
pub mod responses;
/// Shared Server-Sent Events parser for Responses API streams.
pub(crate) mod sse;
/// Shared one-shot HTTP server helpers for provider and integration tests.
#[cfg(test)]
pub(crate) mod test_support;
