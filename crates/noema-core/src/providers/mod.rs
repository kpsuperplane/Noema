//! Concrete provider adapters.

/// Provider adapter that shells out to `codex exec`.
pub mod codex;
/// Provider runtime that keeps a Codex app-server process warm.
pub mod codex_app_server;
/// Provider adapter that shells out to `codex login --device-auth`.
pub mod codex_auth;
/// Provider adapter for the OpenAI Responses API.
pub mod openai;
