//! Late-bound runtime completion adapter for MCP tool classification.

use std::sync::{Arc, RwLock};

use noema_capabilities_mcp::{
    McpCompletionFuture, McpToolClassificationCompletion, McpToolClassificationError,
    McpToolClassificationHandle, McpToolClassificationRequest, McpToolClassificationResponse,
};
use noema_home::{SystemErrorEvent, SystemErrorLogger};

use noema_runtime::RuntimeHandle;

/// Breaks the composition cycle between runtime MCP handles and tool classification.
#[derive(Clone)]
pub(crate) struct RuntimeMcpToolClassificationBridge {
    runtime: Arc<RwLock<Option<RuntimeHandle>>>,
    system_errors: SystemErrorLogger,
}

impl std::fmt::Debug for RuntimeMcpToolClassificationBridge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeMcpToolClassificationBridge")
            .field("runtime", &"[LATE_BOUND]")
            .field("system_errors", &"[CONFIGURED]")
            .finish()
    }
}

impl RuntimeMcpToolClassificationBridge {
    pub(crate) fn new(system_errors: SystemErrorLogger) -> Self {
        Self {
            runtime: Arc::new(RwLock::new(None)),
            system_errors,
        }
    }

    pub(crate) fn handle(&self) -> McpToolClassificationHandle {
        Arc::new(self.clone())
    }

    pub(crate) fn attach(&self, runtime: RuntimeHandle) {
        if let Ok(mut slot) = self.runtime.write() {
            *slot = Some(runtime);
        }
    }

    fn runtime(&self) -> Result<RuntimeHandle, McpToolClassificationError> {
        self.runtime
            .read()
            .map_err(|_| McpToolClassificationError::Failed)?
            .clone()
            .ok_or(McpToolClassificationError::Unavailable)
    }
}

impl McpToolClassificationCompletion for RuntimeMcpToolClassificationBridge {
    fn complete(
        &self,
        request: McpToolClassificationRequest,
    ) -> McpCompletionFuture<'_, Result<McpToolClassificationResponse, McpToolClassificationError>>
    {
        Box::pin(async move {
            let runtime = self.runtime()?;
            let mut generate = noema_providers::GenerateRequest::text(request.prompt);
            generate.instructions = Some(request.instructions);
            generate.options.generation_priority = noema_providers::GenerationPriority::Background;
            match runtime
                .generate_once_with_tool_classification_model(generate)
                .await
            {
                Ok(response) => Ok(McpToolClassificationResponse {
                    assistant_text: response.assistant_text(),
                }),
                Err(error) => {
                    self.system_errors.try_append(
                        SystemErrorEvent::new(
                            noema_capabilities_mcp::SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE,
                            "MCP tool classification failed",
                        )
                        .with_context(serde_json::json!({"operation":"classify_tool"}))
                        .with_error_chain([error.to_string()]),
                    );
                    Err(McpToolClassificationError::Failed)
                }
            }
        })
    }
}
