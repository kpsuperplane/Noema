//! Concrete provider adapters.

/// Codex OAuth token storage and device-code authentication.
pub mod codex_oauth;
/// Provider adapter for Codex direct Responses API.
pub mod codex_responses;
/// Provider adapter for the OpenAI Responses API.
pub mod openai;
/// Shared transport and parser for Responses API providers.
pub mod responses;
