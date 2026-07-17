//! Narrow model-completion port used only for calibration autofill.

use std::{future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

/// Boxed future returned by the object-safe autofill completion port.
pub type McpCompletionFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Metadata-only completion request for MCP calibration suggestions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpAutofillCompletionRequest {
    /// Fixed behavior instruction for strict structured output.
    pub instructions: String,
    /// MCP-owned prompt built from persisted server and tool metadata.
    pub prompt: String,
}

/// Text returned by the injected completion implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpAutofillCompletionResponse {
    /// Assistant text to parse with the MCP-owned strict parser.
    pub assistant_text: String,
}

/// Safe completion failure without provider or transport diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpAutofillCompletionError {
    /// No completion implementation is currently available.
    #[error("MCP calibration autofill is unavailable")]
    Unavailable,
    /// Completion was cancelled by shutdown or its owner.
    #[error("MCP calibration autofill was cancelled")]
    Cancelled,
    /// Completion exceeded its configured deadline.
    #[error("MCP calibration autofill timed out")]
    TimedOut,
    /// Completion failed without safe provider-specific details.
    #[error("MCP calibration autofill failed")]
    Failed,
}

/// Object-safe metadata-only completion boundary for calibration autofill.
pub trait McpAutofillCompletion: Send + Sync {
    /// Generate one strict-text completion for the supplied metadata prompt.
    fn complete(
        &self,
        request: McpAutofillCompletionRequest,
    ) -> McpCompletionFuture<'_, Result<McpAutofillCompletionResponse, McpAutofillCompletionError>>;
}

/// Shared calibration-autofill completion handle.
pub type McpAutofillCompletionHandle = Arc<dyn McpAutofillCompletion>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_trait_remains_dyn_compatible() {
        fn accepts_object_safe_port(_port: Option<&dyn McpAutofillCompletion>) {}

        accepts_object_safe_port(None);
    }
}
