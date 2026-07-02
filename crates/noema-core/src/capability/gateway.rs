//! Runtime Capability Gateway for provider-proposed tool calls.

use crate::{
    McpCalibrationStatus, McpServerAuthStatus, McpServerHealthStatus, McpTransportKind, NoemaStore,
    SYSTEM_ERROR_MCP_MALFORMED_RESPONSE, SystemErrorEvent, SystemErrorLogger,
    mcp::{
        McpClientError, McpClientRuntime, McpTransport, SseMcpTransport, StdioMcpTransport,
        StreamableHttpMcpTransport, secrets::read_mcp_secrets,
    },
};
use serde_json::{Value, json};

/// Runtime gateway facade.
pub struct CapabilityGateway<'a> {
    /// Canonical Noema store used by calibrated capability implementations.
    pub store: &'a NoemaStore,
    /// Developer diagnostic logger for system-level capability failures.
    pub system_errors: &'a SystemErrorLogger,
}

/// Provider-proposed tool call to mediate through the Capability Gateway.
pub struct GatewayToolProposal<'a> {
    /// Provider-visible tool name.
    pub name: &'a str,
    /// Provider-supplied tool payload.
    pub payload: &'a Value,
    /// Agent proposing the tool call.
    pub agent_id: &'a str,
    /// Active governable scope ids for the proposal.
    pub scope_ids: &'a [String],
}

/// Gateway execution result released back into the runtime transcript.
#[derive(Debug, Clone)]
pub struct GatewayToolResult {
    /// Whether the proposed tool completed successfully.
    pub success: bool,
    /// Model-visible result payload.
    pub payload: Value,
    /// Whether this result should be fed back to the provider in the same turn.
    pub requires_provider_continuation: bool,
}

impl CapabilityGateway<'_> {
    /// Execute or deny a provider-proposed tool call.
    pub async fn execute_tool_proposal(
        &self,
        proposal: GatewayToolProposal<'_>,
    ) -> GatewayToolResult {
        let _ = (proposal.agent_id, proposal.scope_ids);

        if let Some(name) = parse_mcp_tool_name(proposal.name) {
            return self.execute_mcp_tool(name, proposal.payload).await;
        }

        GatewayToolResult {
            success: false,
            payload: json!({"error": "unknown_tool"}),
            requires_provider_continuation: false,
        }
    }

    async fn execute_mcp_tool(&self, name: McpToolName<'_>, payload: &Value) -> GatewayToolResult {
        match self.try_execute_mcp_tool(name, payload).await {
            Ok(payload) => GatewayToolResult {
                success: true,
                payload,
                requires_provider_continuation: true,
            },
            Err(error) => GatewayToolResult {
                success: false,
                payload: json!({ "error": error }),
                requires_provider_continuation: false,
            },
        }
    }

    async fn try_execute_mcp_tool(
        &self,
        name: McpToolName<'_>,
        payload: &Value,
    ) -> Result<Value, &'static str> {
        let server = self
            .store
            .get_mcp_server(name.server_id)
            .await
            .map_err(|_| "mcp_store_unavailable")?
            .ok_or("mcp_server_not_found")?;
        if !server.enabled {
            return Err("mcp_server_disabled");
        }
        if server.health_status != McpServerHealthStatus::Healthy {
            return Err("mcp_server_unhealthy");
        }
        if !matches!(
            server.auth_status,
            McpServerAuthStatus::None | McpServerAuthStatus::Authenticated
        ) {
            return Err("mcp_server_auth_required");
        }

        let tool = self
            .store
            .list_mcp_tools_for_server(&server.mcp_server_id)
            .await
            .map_err(|_| "mcp_store_unavailable")?
            .into_iter()
            .find(|tool| tool.name == name.tool_name)
            .ok_or("mcp_tool_not_found")?;
        let calibration = self
            .store
            .get_tool_calibration(&tool.mcp_tool_id)
            .await
            .map_err(|_| "mcp_store_unavailable")?
            .ok_or("mcp_tool_not_calibrated")?;
        if calibration.status != McpCalibrationStatus::Ready
            || calibration.reviewed_metadata_fingerprint.as_deref()
                != Some(tool.metadata_fingerprint.as_str())
        {
            return Err("mcp_tool_not_calibrated");
        }

        let secrets = read_mcp_secrets(&self.store.mcp_server_home(&server.mcp_server_id))
            .unwrap_or_default();
        let arguments = tool_arguments_from_payload(payload)?;
        let result = match server.transport_kind {
            McpTransportKind::Stdio => {
                let transport = StdioMcpTransport::from_server_config(&server, &secrets)
                    .map_err(|_| "mcp_transport_unavailable")?
                    .with_diagnostics(
                        Some(self.system_errors.clone()),
                        Some(server.mcp_server_id.clone()),
                    );
                call_mcp_transport_tool(transport, &tool.name, arguments.clone()).await
            }
            McpTransportKind::Sse => {
                let transport = SseMcpTransport::from_server_config(&server, &secrets)
                    .map_err(|_| "mcp_transport_unavailable")?
                    .with_diagnostics(
                        Some(self.system_errors.clone()),
                        Some(server.mcp_server_id.clone()),
                    );
                call_mcp_transport_tool(transport, &tool.name, arguments.clone()).await
            }
            McpTransportKind::StreamableHttp => {
                let transport = StreamableHttpMcpTransport::from_server_config(&server, &secrets)
                    .map_err(|_| "mcp_transport_unavailable")?
                    .with_diagnostics(
                        Some(self.system_errors.clone()),
                        Some(server.mcp_server_id.clone()),
                    );
                call_mcp_transport_tool(transport, &tool.name, arguments.clone()).await
            }
        };
        result.map_err(|error| self.mcp_tool_call_error(name, payload, &arguments, error))
    }

    fn mcp_tool_call_error(
        &self,
        name: McpToolName<'_>,
        payload: &Value,
        arguments: &Value,
        error: McpClientError,
    ) -> &'static str {
        if matches!(error, McpClientError::Malformed(_)) {
            self.system_errors.try_append(
                SystemErrorEvent::new(SYSTEM_ERROR_MCP_MALFORMED_RESPONSE, error.to_string())
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
        }
        "mcp_tool_call_failed"
    }
}

/// Treat `mcp.<server>.<tool>` names as MCP-shaped for the first gateway slice.
#[must_use]
pub fn is_mcp_shaped_tool_name(name: &str) -> bool {
    parse_mcp_tool_name(name).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct McpToolName<'a> {
    server_id: &'a str,
    tool_name: &'a str,
}

fn parse_mcp_tool_name(name: &str) -> Option<McpToolName<'_>> {
    let mut segments = name.splitn(3, '.');
    if segments.next()? != "mcp" {
        return None;
    }
    let server_id = segments.next()?;
    let tool_name = segments.next()?;
    if server_id.is_empty() || tool_name.is_empty() {
        return None;
    }
    Some(McpToolName {
        server_id,
        tool_name,
    })
}

fn tool_arguments_from_payload(payload: &Value) -> Result<Value, &'static str> {
    match payload.get("arguments") {
        Some(arguments) => Ok(arguments.clone()),
        None => Ok(payload.clone()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        McpCalibrationStatus, McpServerAuthStatus, McpServerHealthStatus, McpTransportKind,
        McpTrustClassification, NewMcpServer, NewMcpTool, NewToolCalibration,
        store::tests::test_store,
    };
    use serde_json::json;

    #[test]
    fn parses_mcp_tool_name_with_colon_server_id() {
        let parsed = parse_mcp_tool_name("mcp.mcp:notion.notion-search").expect("parsed");

        assert_eq!(parsed.server_id, "mcp:notion");
        assert_eq!(parsed.tool_name, "notion-search");
    }

    #[tokio::test]
    async fn gateway_reports_disabled_server_before_calibration() {
        let store = test_store().await;
        seed_mcp_tool(&store, false).await;
        seed_ready_calibration(&store).await;
        set_server_enabled(&store, false).await;
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let system_errors = SystemErrorLogger::new(temp_dir.path().join("errors.log"));
        let gateway = CapabilityGateway {
            store: &store,
            system_errors: &system_errors,
        };

        let result = gateway
            .execute_tool_proposal(GatewayToolProposal {
                name: "mcp.mcp:notion.notion-search",
                payload: &json!({"arguments": {"query": "project"}}),
                agent_id: "agent:primary",
                scope_ids: &["human:local".to_string()],
            })
            .await;

        assert!(!result.success);
        assert_eq!(result.payload["error"], "mcp_server_disabled");
    }

    #[tokio::test]
    async fn gateway_reports_uncalibrated_enabled_tool() {
        let store = test_store().await;
        seed_mcp_tool(&store, true).await;
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let system_errors = SystemErrorLogger::new(temp_dir.path().join("errors.log"));
        let gateway = CapabilityGateway {
            store: &store,
            system_errors: &system_errors,
        };

        let result = gateway
            .execute_tool_proposal(GatewayToolProposal {
                name: "mcp.mcp:notion.notion-search",
                payload: &json!({"arguments": {"query": "project"}}),
                agent_id: "agent:primary",
                scope_ids: &["human:local".to_string()],
            })
            .await;

        assert!(!result.success);
        assert_eq!(result.payload["error"], "mcp_tool_not_calibrated");
    }

    #[test]
    fn mcp_tool_arguments_accept_nested_or_raw_payloads() {
        assert_eq!(
            tool_arguments_from_payload(&json!({"arguments": {"query": "project"}}))
                .expect("nested arguments"),
            json!({"query": "project"})
        );
        assert_eq!(
            tool_arguments_from_payload(&json!({"query": "project"})).expect("raw arguments"),
            json!({"query": "project"})
        );
    }

    async fn seed_mcp_tool(store: &crate::NoemaStore, enabled: bool) {
        store
            .create_mcp_server(NewMcpServer {
                mcp_server_id: "mcp:notion".to_string(),
                display_name: "Notion".to_string(),
                transport_kind: McpTransportKind::StreamableHttp,
                safe_config: json!({"url": "https://mcp.notion.example/mcp"}),
            })
            .await
            .expect("server");
        store
            .update_mcp_server_setup_status(
                "mcp:notion",
                McpServerHealthStatus::Healthy,
                McpServerAuthStatus::Authenticated,
            )
            .await
            .expect("status");
        set_server_enabled(store, enabled).await;
        store
            .upsert_discovered_mcp_tool(NewMcpTool {
                mcp_tool_id: "mcp_tool:mcp_notion:notion-search".to_string(),
                mcp_server_id: "mcp:notion".to_string(),
                name: "notion-search".to_string(),
                description: Some("Search Notion".to_string()),
                input_schema: json!({"type": "object"}),
                output_schema: Some(json!({"type": "object"})),
                annotations: json!({}),
                metadata_fingerprint: "fingerprint:notion-search:v1".to_string(),
            })
            .await
            .expect("tool");
    }

    async fn set_server_enabled(store: &crate::NoemaStore, enabled: bool) {
        store
            .db()
            .query("UPDATE mcp_servers SET enabled = $enabled WHERE mcp_server_id = 'mcp:notion';")
            .bind(("enabled", enabled))
            .await
            .expect("enable query")
            .check()
            .expect("enable server");
    }

    async fn seed_ready_calibration(store: &crate::NoemaStore) {
        store
            .save_tool_calibration(NewToolCalibration {
                calibration_id: "tool_calibration:notion-search".to_string(),
                mcp_tool_id: "mcp_tool:mcp_notion:notion-search".to_string(),
                read_classification: McpTrustClassification::Trusted,
                write_classification: McpTrustClassification::None,
                export_classification: McpTrustClassification::None,
                owner_extractors: Vec::new(),
                status: McpCalibrationStatus::Ready,
                reviewed_by: Some("human:local".to_string()),
                reviewed_metadata_fingerprint: Some("fingerprint:notion-search:v1".to_string()),
            })
            .await
            .expect("calibration");
    }
}
