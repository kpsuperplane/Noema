//! Late-bound runtime completion adapter for MCP calibration autofill.

use std::sync::{Arc, RwLock};

use noema_capabilities_mcp::{
    McpAutofillCompletion, McpAutofillCompletionError, McpAutofillCompletionHandle,
    McpAutofillCompletionRequest, McpAutofillCompletionResponse, McpCompletionFuture,
};
use noema_home::{SystemErrorEvent, SystemErrorLogger};

use noema_runtime::RuntimeHandle;

/// Breaks the composition cycle between runtime MCP handles and autofill.
#[derive(Clone)]
pub(crate) struct RuntimeMcpAutofillCompletionBridge {
    runtime: Arc<RwLock<Option<RuntimeHandle>>>,
    system_errors: SystemErrorLogger,
}

impl std::fmt::Debug for RuntimeMcpAutofillCompletionBridge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeMcpAutofillCompletionBridge")
            .field("runtime", &"[LATE_BOUND]")
            .field("system_errors", &"[CONFIGURED]")
            .finish()
    }
}

impl RuntimeMcpAutofillCompletionBridge {
    pub(crate) fn new(system_errors: SystemErrorLogger) -> Self {
        Self {
            runtime: Arc::new(RwLock::new(None)),
            system_errors,
        }
    }

    pub(crate) fn handle(&self) -> McpAutofillCompletionHandle {
        Arc::new(self.clone())
    }

    pub(crate) fn attach(&self, runtime: RuntimeHandle) {
        if let Ok(mut slot) = self.runtime.write() {
            *slot = Some(runtime);
        }
    }

    fn runtime(&self) -> Result<RuntimeHandle, McpAutofillCompletionError> {
        self.runtime
            .read()
            .map_err(|_| McpAutofillCompletionError::Failed)?
            .clone()
            .ok_or(McpAutofillCompletionError::Unavailable)
    }
}

impl McpAutofillCompletion for RuntimeMcpAutofillCompletionBridge {
    fn complete(
        &self,
        request: McpAutofillCompletionRequest,
    ) -> McpCompletionFuture<'_, Result<McpAutofillCompletionResponse, McpAutofillCompletionError>>
    {
        Box::pin(async move {
            let runtime = self.runtime()?;
            let mut generate = noema_providers::GenerateRequest::text(request.prompt);
            generate.instructions = Some(request.instructions);
            match runtime
                .generate_once_with_tool_classification_model(generate)
                .await
            {
                Ok(response) => Ok(McpAutofillCompletionResponse {
                    assistant_text: response.assistant_text(),
                }),
                Err(error) => {
                    self.system_errors.try_append(
                        SystemErrorEvent::new(
                            noema_capabilities_mcp::SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE,
                            "MCP calibration autofill failed",
                        )
                        .with_context(serde_json::json!({"operation":"autofill"}))
                        .with_error_chain([error.to_string()]),
                    );
                    Err(McpAutofillCompletionError::Failed)
                }
            }
        })
    }
}
