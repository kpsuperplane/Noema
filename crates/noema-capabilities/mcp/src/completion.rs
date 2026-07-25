//! Narrow model-completion port used only for missing tool-hint classification.

use std::{future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

/// Boxed future returned by the object-safe classification completion port.
pub type McpCompletionFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Metadata-only completion request for MCP tool-hint classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolClassificationRequest {
    /// Fixed behavior instruction for strict structured output.
    pub instructions: String,
    /// MCP-owned prompt built from persisted server and tool metadata.
    pub prompt: String,
}

/// Text returned by the injected completion implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolClassificationResponse {
    /// Assistant text to parse with the MCP-owned strict parser.
    pub assistant_text: String,
}

/// Safe completion failure without provider or transport diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpToolClassificationError {
    /// No completion implementation is currently available.
    #[error("MCP tool classification is unavailable")]
    Unavailable,
    /// Completion was cancelled by shutdown or its owner.
    #[error("MCP tool classification was cancelled")]
    Cancelled,
    /// Completion exceeded its configured deadline.
    #[error("MCP tool classification timed out")]
    TimedOut,
    /// Completion failed without safe provider-specific details.
    #[error("MCP tool classification failed")]
    Failed,
}

/// Object-safe metadata-only completion boundary for tool classification.
pub trait McpToolClassificationCompletion: Send + Sync {
    /// Generate one strict-text completion for the supplied metadata prompt.
    fn complete(
        &self,
        request: McpToolClassificationRequest,
    ) -> McpCompletionFuture<'_, Result<McpToolClassificationResponse, McpToolClassificationError>>;
}

/// Shared tool-classification completion handle.
pub type McpToolClassificationHandle = Arc<dyn McpToolClassificationCompletion>;
