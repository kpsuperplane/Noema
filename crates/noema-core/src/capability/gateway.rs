//! Runtime Capability Gateway for provider-proposed tool calls.

use crate::{
    McpServerAuthStatus, McpServerHealthStatus, McpTransportKind, NoemaStore,
    mcp::{
        McpClientError, McpClientRuntime, McpTransport, SYSTEM_ERROR_MCP_MALFORMED_RESPONSE,
        SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE, StdioMcpTransport, StreamableHttpMcpTransport,
        mcp_tool_ineligibility,
        secrets::{
            McpOAuthStoredCredentials, McpSecretMaterial, read_mcp_secrets, write_mcp_secrets,
        },
    },
};
use noema_capabilities::{
    CapabilityError, CapabilityFuture, CapabilityInvocation, CapabilityInvoker, CapabilityOutput,
    OperationToken,
};
use noema_home::{SystemErrorEvent, SystemErrorLogger};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

/// Runtime gateway facade.
#[derive(Clone)]
pub(crate) struct CapabilityGateway {
    /// Canonical Noema store used by calibrated capability implementations.
    pub(crate) store: NoemaStore,
    /// Developer diagnostic logger for system-level capability failures.
    pub(crate) system_errors: SystemErrorLogger,
}

/// Authority captured when one MCP binding is advertised. Invocation rechecks
/// every identity-bearing field so a continuation cannot target replacement
/// metadata or a redirected server connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct McpOperationAuthority {
    canonical_name: String,
    server_id: String,
    server_authority_generation: String,
    tool_id: String,
    tool_name: String,
    metadata_fingerprint: String,
    transport_kind: McpTransportKind,
    safe_config: Value,
    calibration_id: String,
    calibration_status: crate::McpCalibrationStatus,
    read_classification: crate::McpTrustClassification,
    write_classification: crate::McpTrustClassification,
    export_classification: crate::McpTrustClassification,
    reviewed_by: Option<String>,
    reviewed_metadata_fingerprint: Option<String>,
}

impl McpOperationAuthority {
    pub(crate) fn new(
        canonical_name: String,
        server: &crate::McpServerRecord,
        tool: &crate::McpToolRecord,
        calibration: &crate::ToolCalibrationRecord,
    ) -> Self {
        Self {
            canonical_name,
            server_id: server.mcp_server_id.clone(),
            server_authority_generation: server.authority_generation.clone(),
            tool_id: tool.mcp_tool_id.clone(),
            tool_name: tool.name.clone(),
            metadata_fingerprint: tool.metadata_fingerprint.clone(),
            transport_kind: server.transport_kind,
            safe_config: server.safe_config.clone(),
            calibration_id: calibration.calibration_id.clone(),
            calibration_status: calibration.status,
            read_classification: calibration.read_classification,
            write_classification: calibration.write_classification,
            export_classification: calibration.export_classification,
            reviewed_by: calibration.reviewed_by.clone(),
            reviewed_metadata_fingerprint: calibration.reviewed_metadata_fingerprint.clone(),
        }
    }

    pub(crate) fn operation_token(&self) -> OperationToken {
        OperationToken::new(
            serde_json::to_string(self).expect("MCP operation authority is serializable"),
        )
    }

    fn from_token(token: &OperationToken) -> Result<Self, CapabilityError> {
        serde_json::from_str(token.as_str()).map_err(|_| CapabilityError::UnknownOperation)
    }
}

impl CapabilityGateway {
    async fn try_execute_mcp_authority(
        &self,
        authority: &McpOperationAuthority,
        payload: &Value,
    ) -> Result<Value, &'static str> {
        let server = self
            .store
            .get_mcp_server(&authority.server_id)
            .await
            .map_err(|_| "mcp_store_unavailable")?
            .ok_or("mcp_server_not_found")?;
        if server.authority_generation != authority.server_authority_generation
            || server.transport_kind != authority.transport_kind
            || server.safe_config != authority.safe_config
        {
            return Err("mcp_operation_stale");
        }
        let tool = self
            .store
            .list_mcp_tools_for_server(&authority.server_id)
            .await
            .map_err(|_| "mcp_store_unavailable")?
            .into_iter()
            .find(|tool| tool.mcp_tool_id == authority.tool_id)
            .ok_or("mcp_tool_not_found")?;
        if tool.mcp_server_id != authority.server_id
            || tool.name != authority.tool_name
            || tool.metadata_fingerprint != authority.metadata_fingerprint
        {
            return Err("mcp_operation_stale");
        }
        let calibration = self
            .store
            .get_tool_calibration(&authority.tool_id)
            .await
            .map_err(|_| "mcp_store_unavailable")?;
        let Some(calibration) = calibration else {
            return Err("mcp_operation_stale");
        };
        if calibration.calibration_id != authority.calibration_id
            || calibration.status != authority.calibration_status
            || calibration.read_classification != authority.read_classification
            || calibration.write_classification != authority.write_classification
            || calibration.export_classification != authority.export_classification
            || calibration.reviewed_by != authority.reviewed_by
            || calibration.reviewed_metadata_fingerprint != authority.reviewed_metadata_fingerprint
        {
            return Err("mcp_operation_stale");
        }
        if let Some(reason) = mcp_tool_ineligibility(&server, &tool, Some(&calibration)) {
            return Err(reason.gateway_error());
        }

        let server_home = self.store.mcp_server_home(&authority.server_id);
        let mut secrets = read_mcp_secrets(&server_home).map_err(|_| "mcp_secrets_unavailable")?;
        let arguments = invocation_arguments(payload);
        let result = match server.transport_kind {
            McpTransportKind::Stdio => {
                let transport = StdioMcpTransport::from_server_config(&server, &secrets)
                    .map_err(|_| "mcp_transport_unavailable")?
                    .with_diagnostics(
                        Some(self.system_errors.clone()),
                        Some(server.mcp_server_id.clone()),
                    );
                call_mcp_transport_tool(transport, &authority.tool_name, arguments.clone()).await
            }
            McpTransportKind::StreamableHttp => {
                let transport = StreamableHttpMcpTransport::from_server_config(&server, &secrets)
                    .map_err(|_| "mcp_transport_unavailable")?
                    .with_diagnostics(
                        Some(self.system_errors.clone()),
                        Some(server.mcp_server_id.clone()),
                    );
                let (result, transport) = call_mcp_transport_tool_returning_transport(
                    transport,
                    &authority.tool_name,
                    arguments.clone(),
                )
                .await;
                persist_refreshed_oauth_credentials(
                    &server_home,
                    &mut secrets,
                    transport.oauth_credentials(),
                )
                .map_err(|_| "mcp_secrets_unavailable")?;
                result
            }
        };
        match result {
            Ok(payload) => Ok(payload),
            Err(error) => Err(self
                .mcp_tool_call_error(
                    McpToolName {
                        server_id: &authority.server_id,
                        tool_name: &authority.tool_name,
                    },
                    payload,
                    &arguments,
                    error,
                )
                .await),
        }
    }

    async fn mcp_tool_call_error(
        &self,
        name: McpToolName<'_>,
        payload: &Value,
        arguments: &Value,
        error: McpClientError,
    ) -> &'static str {
        self.mark_mcp_server_unhealthy_after_call_failure(name.server_id, &error)
            .await;
        let category = if matches!(error, McpClientError::Malformed(_)) {
            SYSTEM_ERROR_MCP_MALFORMED_RESPONSE
        } else {
            SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE
        };
        self.system_errors.try_append(
            SystemErrorEvent::new(category, error.to_string())
                .with_context(json!({
                    "mcp_server_id": name.server_id,
                    "tool_name": name.tool_name,
                    "method": "tools/call",
                }))
                .with_error_chain([error.to_string()])
                .with_raw(json!({
                    "proposal_payload": payload,
                    "arguments": arguments,
                })),
        );
        mcp_tool_call_gateway_error(&error)
    }

    async fn mark_mcp_server_unhealthy_after_call_failure(
        &self,
        mcp_server_id: &str,
        error: &McpClientError,
    ) {
        let Ok(Some(server)) = self.store.get_mcp_server(mcp_server_id).await else {
            return;
        };
        let auth_status = if mcp_client_error_indicates_auth_failure(error) {
            McpServerAuthStatus::NeedsAuth
        } else {
            server.auth_status
        };
        let _ = self
            .store
            .update_mcp_server_setup_status(
                mcp_server_id,
                McpServerHealthStatus::Unavailable,
                auth_status,
            )
            .await;
    }
}

impl CapabilityInvoker for CapabilityGateway {
    fn invoke(
        &self,
        invocation: CapabilityInvocation,
    ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
        Box::pin(async move {
            let authority = McpOperationAuthority::from_token(&invocation.operation_token)?;
            if invocation.operation.as_str() != authority.canonical_name {
                return Err(CapabilityError::UnknownOperation);
            }
            match self
                .try_execute_mcp_authority(&authority, &invocation.arguments)
                .await
            {
                Ok(payload) if mcp_tool_payload_is_error(&payload) => {
                    Ok(CapabilityOutput::failed(payload))
                }
                Ok(payload) => Ok(CapabilityOutput::success(payload)),
                Err("mcp_operation_stale" | "mcp_server_not_found" | "mcp_tool_not_found") => {
                    Err(CapabilityError::UnknownOperation)
                }
                Err(
                    "mcp_server_disabled"
                    | "mcp_tool_disabled"
                    | "mcp_server_unhealthy"
                    | "mcp_server_auth_required"
                    | "mcp_tool_not_calibrated"
                    | "mcp_tool_approval_required",
                ) => Err(CapabilityError::Denied),
                Err("mcp_store_unavailable" | "mcp_secrets_unavailable") => {
                    Err(CapabilityError::Unavailable)
                }
                Err(_) => Err(CapabilityError::Failed),
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct McpToolName<'a> {
    server_id: &'a str,
    tool_name: &'a str,
}

fn invocation_arguments(payload: &Value) -> Value {
    payload.clone()
}

fn mcp_tool_call_gateway_error(error: &McpClientError) -> &'static str {
    if mcp_client_error_indicates_auth_failure(error) {
        "mcp_authentication_failed"
    } else {
        "mcp_tool_call_failed"
    }
}

fn mcp_client_error_indicates_auth_failure(error: &McpClientError) -> bool {
    match error {
        McpClientError::AuthRequired(_) => true,
        McpClientError::Transport(message) => transport_error_indicates_auth_failure(message),
        McpClientError::Malformed(_) => false,
    }
}

fn mcp_tool_payload_is_error(payload: &Value) -> bool {
    payload
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn transport_error_indicates_auth_failure(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("unauthorized")
        || normalized.contains("authentication failed")
        || normalized.contains("401")
        || normalized.contains("403")
}

async fn call_mcp_transport_tool<T>(
    transport: T,
    tool_name: &str,
    arguments: Value,
) -> Result<Value, McpClientError>
where
    T: McpTransport,
{
    let mut runtime = McpClientRuntime::new(transport);
    runtime.call_tool(tool_name, arguments).await
}

async fn call_mcp_transport_tool_returning_transport<T>(
    transport: T,
    tool_name: &str,
    arguments: Value,
) -> (Result<Value, McpClientError>, T)
where
    T: McpTransport,
{
    let mut runtime = McpClientRuntime::new(transport);
    let result = runtime.call_tool(tool_name, arguments).await;
    (result, runtime.into_inner())
}

fn persist_refreshed_oauth_credentials(
    server_home: &Path,
    secrets: &mut McpSecretMaterial,
    credentials: Option<&McpOAuthStoredCredentials>,
) -> std::io::Result<()> {
    let Some(credentials) = credentials else {
        return Ok(());
    };
    if secrets.oauth_credentials.as_ref() == Some(credentials) {
        return Ok(());
    }
    secrets.oauth_credentials = Some(credentials.clone());
    write_mcp_secrets(server_home, secrets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        McpCalibrationStatus, McpTrustClassification, NewMcpServer, NewMcpTool, NewToolCalibration,
        store::tests::test_store,
    };
    use noema_capabilities::ToolName;
    use serde_json::json;
    use std::path::Path;

    fn read_system_error_events(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .expect("system error log")
            .lines()
            .map(|line| serde_json::from_str(line).expect("system error event"))
            .collect()
    }

    #[tokio::test]
    async fn exact_operation_token_rejects_a_different_canonical_name() {
        let store = test_store().await;
        let authority = seed_ready_tool(&store).await;
        let gateway = test_gateway(store);
        let error = gateway
            .invoke(CapabilityInvocation {
                operation: ToolName::new("mcp.mcp:docs.other").expect("name"),
                operation_token: authority.operation_token(),
                arguments: json!({}),
            })
            .await
            .expect_err("different operation denied");
        assert_eq!(error, CapabilityError::UnknownOperation);
    }

    #[tokio::test]
    async fn authority_round_trips_colon_server_id() {
        let store = test_store().await;
        let authority = seed_ready_tool(&store).await;

        let round_tripped = McpOperationAuthority::from_token(&authority.operation_token())
            .expect("captured authority");

        assert_eq!(round_tripped.server_id, "mcp:docs");
        assert_eq!(round_tripped.tool_name, "read");
    }

    #[test]
    fn invocation_forwards_a_legitimate_arguments_property_exactly() {
        let payload = json!({
            "arguments": {"nested": true},
            "ordinary": "sibling field"
        });
        assert_eq!(invocation_arguments(&payload), payload);
    }

    #[tokio::test]
    async fn malformed_operation_token_is_unknown_without_store_lookup() {
        let gateway = test_gateway(test_store().await);
        let error = gateway
            .invoke(CapabilityInvocation {
                operation: ToolName::new("mcp.mcp:docs.read").expect("name"),
                operation_token: OperationToken::new("not-json"),
                arguments: json!({"secret": "must not be parsed as authority"}),
            })
            .await
            .expect_err("malformed token denied");
        assert_eq!(error, CapabilityError::UnknownOperation);
    }

    #[tokio::test]
    async fn old_authority_is_stale_after_connection_identity_rotation() {
        let store = test_store().await;
        let authority = seed_ready_tool(&store).await;
        store
            .update_mcp_server_connection_identity(
                "mcp:docs",
                McpTransportKind::Stdio,
                json!({"command": "replacement"}),
            )
            .await
            .expect("rotate connection");
        let error = invoke_authority(&test_gateway(store), &authority)
            .await
            .expect_err("old authority denied");
        assert_eq!(error, CapabilityError::UnknownOperation);
    }

    #[tokio::test]
    async fn old_authority_is_stale_after_identical_delete_and_recreate() {
        let store = test_store().await;
        let authority = seed_ready_tool(&store).await;
        let old_generation = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server")
            .authority_generation;
        assert!(store.delete_mcp_server("mcp:docs").await.expect("delete"));
        let replacement = seed_ready_tool(&store).await;
        let new_generation = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server")
            .authority_generation;
        assert_ne!(old_generation, new_generation);
        assert_ne!(authority.operation_token(), replacement.operation_token());

        let error = invoke_authority(&test_gateway(store), &authority)
            .await
            .expect_err("old authority denied");
        assert_eq!(error, CapabilityError::UnknownOperation);
    }

    #[tokio::test]
    async fn health_and_auth_updates_do_not_rotate_connection_generation() {
        let store = test_store().await;
        seed_ready_tool(&store).await;
        let before = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server")
            .authority_generation;
        store
            .update_mcp_server_setup_status(
                "mcp:docs",
                McpServerHealthStatus::Unavailable,
                McpServerAuthStatus::NeedsAuth,
            )
            .await
            .expect("status");
        let after = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server")
            .authority_generation;
        assert_eq!(before, after);
    }

    #[tokio::test]
    async fn identical_connection_identity_update_keeps_generation_and_token() {
        let store = test_store().await;
        let authority = seed_ready_tool(&store).await;
        let before = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server")
            .authority_generation;
        store
            .update_mcp_server_connection_identity(
                "mcp:docs",
                McpTransportKind::Stdio,
                json!({"command": "docs-server"}),
            )
            .await
            .expect("identity update");
        let server = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server");
        let tool = store
            .list_mcp_tools_for_server("mcp:docs")
            .await
            .expect("tools")
            .into_iter()
            .next()
            .expect("tool");
        let calibration = store
            .get_tool_calibration(&tool.mcp_tool_id)
            .await
            .expect("calibration read")
            .expect("calibration");
        let refreshed = McpOperationAuthority::new(
            "mcp.mcp:docs.read".to_string(),
            &server,
            &tool,
            &calibration,
        );

        assert_eq!(before, server.authority_generation);
        assert_eq!(authority.operation_token(), refreshed.operation_token());
    }

    #[tokio::test]
    async fn live_health_revocation_denies_an_exact_retained_authority() {
        let store = test_store().await;
        let authority = seed_ready_tool(&store).await;
        store
            .update_mcp_server_setup_status(
                "mcp:docs",
                McpServerHealthStatus::Unavailable,
                McpServerAuthStatus::None,
            )
            .await
            .expect("status");
        let error = invoke_authority(&test_gateway(store), &authority)
            .await
            .expect_err("unhealthy server denied");
        assert_eq!(error, CapabilityError::Denied);
    }

    #[tokio::test]
    async fn gateway_reports_disabled_server_before_calibration() {
        let store = test_store().await;
        let authority = seed_ready_tool(&store).await;
        store
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE mcp_servers SET enabled = 0 WHERE mcp_server_id = 'mcp:docs'",
                    [],
                )?;
                Ok(())
            })
            .await
            .expect("disable server");

        let error = invoke_authority(&test_gateway(store), &authority)
            .await
            .expect_err("disabled server denied");

        assert_eq!(error, CapabilityError::Denied);
    }

    #[test]
    fn mcp_tool_payload_is_error_marks_result_failed() {
        assert!(mcp_tool_payload_is_error(&json!({"isError": true})));
        assert!(!mcp_tool_payload_is_error(&json!({"isError": false})));
        assert!(!mcp_tool_payload_is_error(&json!({"content": []})));
    }

    #[tokio::test]
    async fn gateway_reports_auth_required_mcp_call_errors() {
        let store = test_store().await;
        seed_ready_tool(&store).await;
        let gateway = test_gateway(store.clone());

        let error = gateway
            .mcp_tool_call_error(
                McpToolName {
                    server_id: "mcp:docs",
                    tool_name: "read",
                },
                &json!({"document_id": "doc_1"}),
                &json!({"document_id": "doc_1"}),
                McpClientError::AuthRequired("MCP server requires authentication".to_string()),
            )
            .await;

        assert_eq!(error, "mcp_authentication_failed");
        let server = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server");
        assert_eq!(server.health_status, McpServerHealthStatus::Unavailable);
        assert_eq!(server.auth_status, McpServerAuthStatus::NeedsAuth);
    }

    #[tokio::test]
    async fn gateway_reports_auth_shaped_mcp_transport_errors() {
        let store = test_store().await;
        seed_ready_tool(&store).await;
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let errors_log_path = temp_dir.path().join("errors.log");
        let gateway = CapabilityGateway {
            store: store.clone(),
            system_errors: SystemErrorLogger::new(&errors_log_path),
        };

        let error = gateway
            .mcp_tool_call_error(
                McpToolName {
                    server_id: "mcp:docs",
                    tool_name: "read",
                },
                &json!({"document_id": "doc_1"}),
                &json!({"document_id": "doc_1"}),
                McpClientError::Transport(
                    "MCP tools/call failed: unauthorized: Authentication failed".to_string(),
                ),
            )
            .await;

        assert_eq!(error, "mcp_authentication_failed");
        let events = read_system_error_events(&errors_log_path);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["category"], SYSTEM_ERROR_MCP_TOOL_CALL_FAILURE);
        assert_eq!(events[0]["context"]["mcp_server_id"], "mcp:docs");
        assert_eq!(events[0]["context"]["tool_name"], "read");
        let server = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server");
        assert_eq!(server.health_status, McpServerHealthStatus::Unavailable);
        assert_eq!(server.auth_status, McpServerAuthStatus::NeedsAuth);
    }

    #[tokio::test]
    async fn gateway_marks_transport_failures_unhealthy_without_requiring_auth() {
        let store = test_store().await;
        seed_ready_tool(&store).await;
        let gateway = test_gateway(store.clone());

        let error = gateway
            .mcp_tool_call_error(
                McpToolName {
                    server_id: "mcp:docs",
                    tool_name: "read",
                },
                &json!({"document_id": "doc_1"}),
                &json!({"document_id": "doc_1"}),
                McpClientError::Transport(
                    "MCP initialize failed: HTTP 500 Internal Server Error".to_string(),
                ),
            )
            .await;

        assert_eq!(error, "mcp_tool_call_failed");
        let server = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server");
        assert_eq!(server.health_status, McpServerHealthStatus::Unavailable);
        assert_eq!(server.auth_status, McpServerAuthStatus::None);
    }

    #[tokio::test]
    async fn calibration_replacement_with_same_id_and_fingerprint_is_stale() {
        let store = test_store().await;
        let authority = seed_ready_tool(&store).await;
        store
            .save_tool_calibration(NewToolCalibration {
                calibration_id: "calibration:docs:read".to_string(),
                mcp_tool_id: "mcp_tool:docs:read".to_string(),
                read_classification: McpTrustClassification::Trusted,
                write_classification: McpTrustClassification::None,
                export_classification: McpTrustClassification::None,
                status: McpCalibrationStatus::Ready,
                reviewed_by: Some("human:replacement-reviewer".to_string()),
                reviewed_metadata_fingerprint: Some("fingerprint:docs:read".to_string()),
            })
            .await
            .expect("replace calibration");
        let error = invoke_authority(&test_gateway(store), &authority)
            .await
            .expect_err("old calibration denied");
        assert_eq!(error, CapabilityError::UnknownOperation);
    }

    async fn invoke_authority(
        gateway: &CapabilityGateway,
        authority: &McpOperationAuthority,
    ) -> Result<CapabilityOutput, CapabilityError> {
        gateway
            .invoke(CapabilityInvocation {
                operation: ToolName::new("mcp.mcp:docs.read").expect("name"),
                operation_token: authority.operation_token(),
                arguments: json!({}),
            })
            .await
    }

    fn test_gateway(store: NoemaStore) -> CapabilityGateway {
        let temp = tempfile::tempdir().expect("temp directory");
        CapabilityGateway {
            store,
            system_errors: SystemErrorLogger::new(temp.keep().join("errors.log")),
        }
    }

    async fn seed_ready_tool(store: &NoemaStore) -> McpOperationAuthority {
        store
            .create_mcp_server(NewMcpServer {
                mcp_server_id: "mcp:docs".to_string(),
                display_name: "Docs".to_string(),
                transport_kind: McpTransportKind::Stdio,
                safe_config: json!({"command": "docs-server"}),
            })
            .await
            .expect("server");
        store
            .upsert_discovered_mcp_tool(NewMcpTool {
                mcp_tool_id: "mcp_tool:docs:read".to_string(),
                mcp_server_id: "mcp:docs".to_string(),
                name: "read".to_string(),
                description: Some("Read docs.".to_string()),
                input_schema: json!({"type": "object"}),
                output_schema: Some(json!({"type": "object"})),
                annotations: json!({}),
                metadata_fingerprint: "fingerprint:docs:read".to_string(),
            })
            .await
            .expect("tool");
        store
            .save_tool_calibration(NewToolCalibration {
                calibration_id: "calibration:docs:read".to_string(),
                mcp_tool_id: "mcp_tool:docs:read".to_string(),
                read_classification: McpTrustClassification::Trusted,
                write_classification: McpTrustClassification::None,
                export_classification: McpTrustClassification::None,
                status: McpCalibrationStatus::Ready,
                reviewed_by: Some("human:reviewer".to_string()),
                reviewed_metadata_fingerprint: Some("fingerprint:docs:read".to_string()),
            })
            .await
            .expect("calibration");
        store
            .update_mcp_server_setup_status(
                "mcp:docs",
                McpServerHealthStatus::Healthy,
                McpServerAuthStatus::None,
            )
            .await
            .expect("status");
        let server = store
            .get_mcp_server("mcp:docs")
            .await
            .expect("server read")
            .expect("server");
        let tool = store
            .list_mcp_tools_for_server("mcp:docs")
            .await
            .expect("tools")
            .into_iter()
            .next()
            .expect("tool");
        let calibration = store
            .get_tool_calibration(&tool.mcp_tool_id)
            .await
            .expect("calibration read")
            .expect("calibration");
        McpOperationAuthority::new(
            "mcp.mcp:docs.read".to_string(),
            &server,
            &tool,
            &calibration,
        )
    }
}
