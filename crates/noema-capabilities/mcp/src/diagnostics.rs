use std::{fmt, sync::Arc};

use noema_home::{SystemErrorEvent, SystemErrorLogger};
use serde_json::{Value, json};

use crate::{
    SYSTEM_ERROR_MCP_MALFORMED_RESPONSE, SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE,
    limits::{MAX_DIAGNOSTIC_RAW_BYTES, bounded_diagnostic_text, json_within_limits},
};

/// Internal MCP diagnostic category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpDiagnosticKind {
    /// A remote protocol payload could not be decoded or validated.
    MalformedResponse,
    /// Setup, discovery, OAuth, secret cleanup, or invocation failed.
    OperationFailure,
}

impl McpDiagnosticKind {
    const fn category(self) -> &'static str {
        match self {
            Self::MalformedResponse => SYSTEM_ERROR_MCP_MALFORMED_RESPONSE,
            Self::OperationFailure => SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE,
        }
    }
}

/// Raw developer-only MCP diagnostic event.
#[derive(Debug, Clone, PartialEq)]
pub struct McpDiagnosticEvent {
    /// Stable diagnostic category.
    pub kind: McpDiagnosticKind,
    /// Sanitized summary suitable for the primary event message.
    pub message: String,
    /// Durable server id, when known.
    pub mcp_server_id: Option<String>,
    /// MCP tool name, when relevant.
    pub tool_name: Option<String>,
    /// Protocol/control-plane operation.
    pub operation: &'static str,
    /// Internal error chain. This is written only to the developer log.
    pub error_chain: Vec<String>,
    /// Raw provider/transport material written only to the developer log.
    pub raw: Option<Value>,
}

/// Narrow synchronous diagnostic sink used by MCP orchestration.
pub trait McpDiagnosticSink: Send + Sync + fmt::Debug {
    /// Record one best-effort developer diagnostic.
    fn record(&self, event: McpDiagnosticEvent);
}

/// Shared MCP diagnostic sink.
pub type McpDiagnosticHandle = Arc<dyn McpDiagnosticSink>;

/// `errors.log` diagnostic sink backed by `noema-home`.
#[derive(Clone)]
pub struct SystemErrorMcpDiagnostics {
    logger: SystemErrorLogger,
}

impl SystemErrorMcpDiagnostics {
    /// Construct an MCP diagnostic sink over the resolved system-error logger.
    #[must_use]
    pub const fn new(logger: SystemErrorLogger) -> Self {
        Self { logger }
    }

    /// Return this sink behind the shared object-safe handle.
    #[must_use]
    pub fn handle(self) -> McpDiagnosticHandle {
        Arc::new(self)
    }
}

impl fmt::Debug for SystemErrorMcpDiagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SystemErrorMcpDiagnostics")
            .field("logger", &"[CONFIGURED]")
            .finish()
    }
}

impl McpDiagnosticSink for SystemErrorMcpDiagnostics {
    fn record(&self, event: McpDiagnosticEvent) {
        let mut diagnostic = SystemErrorEvent::new(event.kind.category(), event.message)
            .with_context(json!({
                "mcp_server_id": event.mcp_server_id,
                "tool_name": event.tool_name,
                "operation": event.operation,
            }))
            .with_error_chain(event.error_chain.into_iter().map(bounded_diagnostic_text));
        if let Some(raw) = event.raw {
            diagnostic =
                diagnostic.with_raw(if json_within_limits(&raw, MAX_DIAGNOSTIC_RAW_BYTES) {
                    raw
                } else {
                    json!({"payload_omitted": true, "reason": "diagnostic_limit"})
                });
        }
        self.logger.try_append(diagnostic);
    }
}
