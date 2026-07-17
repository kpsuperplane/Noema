//! MCP capability invocation with live authority and policy revalidation.

use noema_capabilities::{
    CapabilityError, CapabilityFuture, CapabilityInvocation, CapabilityInvoker, CapabilityOutput,
};
use serde_json::Value;

use crate::{
    LocalMcpService, McpClientError, McpDiagnosticEvent, McpDiagnosticKind, McpFailureStatus,
    McpOperationAuthority, McpServerAuthStatus, McpServerHealthStatus, McpToolCallOutput,
    mcp_tool_ineligibility,
    service::map_client_operation_error,
    setup::{auth_status_for_secrets, secret_material_matches_server},
};

impl CapabilityInvoker for LocalMcpService {
    fn invoke(
        &self,
        invocation: CapabilityInvocation,
    ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
        Box::pin(async move { self.invoke_capability(invocation).await })
    }
}

impl LocalMcpService {
    async fn invoke_capability(
        &self,
        invocation: CapabilityInvocation,
    ) -> Result<CapabilityOutput, CapabilityError> {
        self.inner
            .lifecycle
            .run_admitted(self.invoke_admitted(invocation))
            .await
            .map_err(|_| CapabilityError::Unavailable)?
    }

    async fn invoke_admitted(
        &self,
        invocation: CapabilityInvocation,
    ) -> Result<CapabilityOutput, CapabilityError> {
        if !invocation.arguments.is_object() && !invocation.arguments.is_null() {
            return Err(CapabilityError::InvalidArguments);
        }
        let authority = McpOperationAuthority::from_operation_token(&invocation.operation_token)?;
        if invocation.operation.as_str() != authority.canonical_name() {
            return Err(CapabilityError::UnknownOperation);
        }

        let context = self
            .inner
            .request_context(self.inner.config.invocation_timeout);
        let _server_guard = self
            .inner
            .lock_server(authority.server_id(), &context)
            .await
            .map_err(|_| CapabilityError::Unavailable)?;
        let _policy_guard = self
            .inner
            .lock_policy_read(authority.server_id(), &context)
            .await
            .map_err(|_| CapabilityError::Unavailable)?;
        let snapshot = self
            .inner
            .repository
            .invocation_snapshot(
                authority.server_id().to_string(),
                authority.tool_id().to_string(),
            )
            .await
            .map_err(|_| CapabilityError::Unavailable)?
            .ok_or(CapabilityError::UnknownOperation)?;
        let calibration = snapshot
            .calibration
            .as_ref()
            .ok_or(CapabilityError::UnknownOperation)?;
        let expected_name = format!(
            "mcp.{}.{}",
            snapshot.server.mcp_server_id, snapshot.tool.name
        );
        if expected_name != authority.canonical_name()
            || !authority.matches(&snapshot.server, &snapshot.tool, calibration)
        {
            return Err(CapabilityError::UnknownOperation);
        }
        if mcp_tool_ineligibility(&snapshot.server, &snapshot.tool, Some(calibration)).is_some() {
            return Err(CapabilityError::Denied);
        }

        let mut secrets = self
            .inner
            .secrets
            .load(&snapshot.server.mcp_server_id)
            .map_err(|_| CapabilityError::Unavailable)?;
        if !secret_material_matches_server(&snapshot.server, &secrets) {
            self.record_missing_secret_failure(&snapshot).await;
            return Err(CapabilityError::Unavailable);
        }
        let preparation = match self
            .inner
            .sessions
            .prepare(&snapshot.server, &secrets, &context)
            .await
        {
            Ok(preparation) => preparation,
            Err(error) => {
                return Err(self
                    .record_invocation_client_failure(&snapshot, &invocation.arguments, &error)
                    .await);
            }
        };
        let (mut session, refreshed_credentials) = preparation.into_parts();
        if let Some(credentials) = refreshed_credentials {
            secrets.oauth_credentials = Some(credentials);
            let stage = match self.inner.secrets.stage(&secrets) {
                Ok(stage) => stage,
                Err(error) => {
                    self.inner.record_failure(
                        Some(&snapshot.server.mcp_server_id),
                        Some(&snapshot.tool.name),
                        "credentials/stage",
                        "MCP credential refresh could not be staged",
                        &error,
                    );
                    self.inner
                        .close_session(
                            session,
                            Some(&snapshot.server.mcp_server_id),
                            "credentials/stage",
                        )
                        .await;
                    return Err(CapabilityError::Unavailable);
                }
            };
            let commit = match self
                .inner
                .secrets
                .commit(stage, &snapshot.server.mcp_server_id)
            {
                Ok(commit) => commit,
                Err(error) => {
                    self.inner.record_failure(
                        Some(&snapshot.server.mcp_server_id),
                        Some(&snapshot.tool.name),
                        "credentials/commit",
                        "MCP credential refresh could not be persisted",
                        &error,
                    );
                    self.inner
                        .close_session(
                            session,
                            Some(&snapshot.server.mcp_server_id),
                            "credentials/commit",
                        )
                        .await;
                    return Err(CapabilityError::Unavailable);
                }
            };
            if let Err(error) = self.inner.secrets.finalize(commit) {
                self.inner.record_failure(
                    Some(&snapshot.server.mcp_server_id),
                    Some(&snapshot.tool.name),
                    "credentials/finalize",
                    "MCP credential backup cleanup failed",
                    &error,
                );
            }
        }

        let result = session
            .call_tool(&snapshot.tool.name, invocation.arguments.clone(), &context)
            .await;
        self.inner
            .close_session(session, Some(&snapshot.server.mcp_server_id), "tools/call")
            .await;
        match result {
            Ok(output) => {
                self.record_invocation_success(&snapshot, &secrets).await;
                Ok(capability_output(output))
            }
            Err(error) => Err(self
                .record_invocation_client_failure(&snapshot, &invocation.arguments, &error)
                .await),
        }
    }

    async fn record_invocation_client_failure(
        &self,
        snapshot: &crate::McpInvocationSnapshot,
        arguments: &Value,
        error: &McpClientError,
    ) -> CapabilityError {
        self.record_invocation_failure(snapshot, arguments, error);
        self.record_invocation_failure_status(snapshot, error).await;
        capability_error_from_client(error)
    }

    fn record_invocation_failure(
        &self,
        snapshot: &crate::McpInvocationSnapshot,
        _arguments: &Value,
        error: &McpClientError,
    ) {
        self.inner.diagnostics.record(McpDiagnosticEvent {
            kind: if matches!(error, McpClientError::Malformed(_)) {
                McpDiagnosticKind::MalformedResponse
            } else {
                McpDiagnosticKind::OperationFailure
            },
            message: "MCP tool invocation failed".to_string(),
            mcp_server_id: Some(snapshot.server.mcp_server_id.clone()),
            tool_name: Some(snapshot.tool.name.clone()),
            operation: "tools/call",
            error_chain: vec![crate::limits::bounded_diagnostic_text(error.to_string())],
            raw: Some(serde_json::json!({"arguments_omitted": true})),
        });
    }

    async fn record_invocation_failure_status(
        &self,
        snapshot: &crate::McpInvocationSnapshot,
        error: &McpClientError,
    ) {
        let auth_status = if matches!(error, McpClientError::AuthenticationRequired(_)) {
            McpServerAuthStatus::NeedsAuth
        } else {
            snapshot.server.auth_status
        };
        let status = McpFailureStatus {
            mcp_server_id: snapshot.server.mcp_server_id.clone(),
            expected_authority_generation: snapshot.server.authority_generation.clone(),
            health_status: McpServerHealthStatus::Unavailable,
            auth_status,
        };
        if let Err(repository_error) = self.inner.repository.record_failure_status(status).await {
            self.inner.record_failure(
                Some(&snapshot.server.mcp_server_id),
                Some(&snapshot.tool.name),
                "status/failure",
                "MCP invocation status could not be recorded",
                &repository_error,
            );
        }
    }

    async fn record_missing_secret_failure(&self, snapshot: &crate::McpInvocationSnapshot) {
        self.inner.record_failure(
            Some(&snapshot.server.mcp_server_id),
            Some(&snapshot.tool.name),
            "credentials/load",
            "MCP credentials are missing or inconsistent",
            &"persisted secret material did not match the active connection",
        );
        let status = McpFailureStatus {
            mcp_server_id: snapshot.server.mcp_server_id.clone(),
            expected_authority_generation: snapshot.server.authority_generation.clone(),
            health_status: McpServerHealthStatus::Unavailable,
            auth_status: McpServerAuthStatus::NeedsAuth,
        };
        if let Err(error) = self.inner.repository.record_failure_status(status).await {
            self.inner.record_failure(
                Some(&snapshot.server.mcp_server_id),
                Some(&snapshot.tool.name),
                "status/credentials",
                "MCP credential failure status could not be recorded",
                &error,
            );
        }
    }

    async fn record_invocation_success(
        &self,
        snapshot: &crate::McpInvocationSnapshot,
        secrets: &crate::McpSecretMaterial,
    ) {
        let status = McpFailureStatus {
            mcp_server_id: snapshot.server.mcp_server_id.clone(),
            expected_authority_generation: snapshot.server.authority_generation.clone(),
            health_status: McpServerHealthStatus::Healthy,
            auth_status: auth_status_for_secrets(secrets),
        };
        if let Err(repository_error) = self.inner.repository.record_failure_status(status).await {
            self.inner.record_failure(
                Some(&snapshot.server.mcp_server_id),
                Some(&snapshot.tool.name),
                "status/success",
                "MCP invocation status could not be recorded",
                &repository_error,
            );
        }
    }
}

fn capability_output(output: McpToolCallOutput) -> CapabilityOutput {
    if output.is_error {
        CapabilityOutput::failed(output.result)
    } else {
        CapabilityOutput::success(output.result)
    }
}

fn capability_error_from_client(error: &McpClientError) -> CapabilityError {
    match map_client_operation_error(error) {
        crate::McpOperationError::MalformedResponse | crate::McpOperationError::Failed => {
            CapabilityError::Failed
        }
        crate::McpOperationError::InvalidInput => CapabilityError::InvalidArguments,
        crate::McpOperationError::AuthenticationRequired
        | crate::McpOperationError::Unavailable
        | crate::McpOperationError::Cancelled
        | crate::McpOperationError::TimedOut
        | crate::McpOperationError::ShuttingDown => CapabilityError::Unavailable,
        crate::McpOperationError::NotFound | crate::McpOperationError::Conflict => {
            CapabilityError::Denied
        }
    }
}

#[cfg(test)]
mod tests;
