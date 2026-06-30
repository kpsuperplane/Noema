//! Concrete provider adapters and transport helpers.

/// Codex OAuth token storage and device-code authentication.
pub mod codex_oauth;
/// Provider adapter for Codex direct Responses API.
pub mod codex_responses;
/// Parser for structured Noema JSON response deltas.
pub(crate) mod noema_response_stream;
/// Provider adapter for the OpenAI Responses API.
pub mod openai;
/// Shared transport and parser for Responses API providers.
pub mod responses;
/// Shared Server-Sent Events parser for Responses API streams.
pub(crate) mod sse;
