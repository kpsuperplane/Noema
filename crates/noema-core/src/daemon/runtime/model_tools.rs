use crate::{
    McpServerAuthStatus, McpServerHealthStatus, NoemaStore,
    daemon::{
        agent_name_tool::update_own_name_tool_spec,
        memory::tool::search_memory_tool_spec,
        runtime::turn::{mcp_auth_status_label, mcp_health_status_label},
    },
    mcp::mcp_tool_ineligibility,
    provider::{
        NoemaToolExecution, NoemaToolSpec, ProviderToolCapabilities, ProviderToolFallbackMode,
        ToolContractError,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ModelTools {
    pub(super) native: Vec<NoemaToolSpec>,
    pub(super) legacy_builtin_envelope_tools: Vec<String>,
    pub(super) prompt_rows: Vec<String>,
    pub(super) unavailable_rows: Vec<String>,
}

pub(super) async fn build_model_tools(
    store: &NoemaStore,
    include_agent_name_tool: bool,
    capabilities: ProviderToolCapabilities,
) -> Result<ModelTools, ToolContractError> {
    let builtin_tools = builtin_tool_specs(include_agent_name_tool)?;
    let unavailable_rows = unavailable_mcp_rows(store).await?;

    if capabilities.native_tools {
        let mut native = builtin_tools.clone();
        native.extend(calibrated_mcp_tool_specs(store).await?);
        return Ok(ModelTools {
            prompt_rows: prompt_rows(&native, &unavailable_rows),
            native,
            legacy_builtin_envelope_tools: Vec::new(),
            unavailable_rows,
        });
    }

    let legacy_builtin_envelope_tools =
        if capabilities.fallback_mode == ProviderToolFallbackMode::BuiltinOnlyEnvelope {
            builtin_tools
                .iter()
                .map(|tool| tool.name.as_str().to_string())
                .collect()
        } else {
            Vec::new()
        };

    Ok(ModelTools {
        native: Vec::new(),
        prompt_rows: prompt_rows(&builtin_tools, &unavailable_rows),
        legacy_builtin_envelope_tools,
        unavailable_rows,
    })
}

fn builtin_tool_specs(
    include_agent_name_tool: bool,
) -> Result<Vec<NoemaToolSpec>, ToolContractError> {
    let mut specs = vec![search_memory_tool_spec()?];
    if include_agent_name_tool {
        specs.push(update_own_name_tool_spec()?);
    }
    Ok(specs)
}

async fn calibrated_mcp_tool_specs(
    store: &NoemaStore,
) -> Result<Vec<NoemaToolSpec>, ToolContractError> {
    let mut specs = Vec::new();
    for server in store.list_mcp_servers().await.map_err(store_tool_error)? {
        let tools = store
            .list_mcp_tools_for_server(&server.mcp_server_id)
            .await
            .map_err(store_tool_error)?;
        for tool in tools {
            let calibration = store
                .get_tool_calibration(&tool.mcp_tool_id)
                .await
                .map_err(store_tool_error)?;
            if mcp_tool_ineligibility(&server, &tool, calibration.as_ref()).is_some() {
                continue;
            }

            let name = format!("mcp.{}.{}", server.mcp_server_id, tool.name);
            let description = tool
                .description
                .as_deref()
                .map(str::trim)
                .filter(|description| !description.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    format!("Call MCP tool {} on {}", tool.name, server.display_name)
                });
            specs.push(NoemaToolSpec::new(
                name,
                description,
                tool.input_schema.clone(),
                NoemaToolExecution::Mcp {
                    server_id: server.mcp_server_id.clone(),
                    tool_name: tool.name,
                    tool_id: tool.mcp_tool_id,
                },
            )?);
        }
    }
    Ok(specs)
}

async fn unavailable_mcp_rows(store: &NoemaStore) -> Result<Vec<String>, ToolContractError> {
    let servers = store.list_mcp_servers().await.map_err(store_tool_error)?;
    Ok(servers
        .into_iter()
        .filter(|server| {
            server.enabled
                && (server.health_status != McpServerHealthStatus::Healthy
                    || !matches!(
                        server.auth_status,
                        McpServerAuthStatus::None | McpServerAuthStatus::Authenticated
                    ))
        })
        .map(|server| {
            format!(
                "- unavailable_mcp\t{}\t{}\thealth={}\tauth={}",
                server.mcp_server_id,
                server.display_name,
                mcp_health_status_label(server.health_status),
                mcp_auth_status_label(server.auth_status),
            )
        })
        .collect())
}

fn prompt_rows(native_tools: &[NoemaToolSpec], unavailable_rows: &[String]) -> Vec<String> {
    let mut rows = Vec::with_capacity(native_tools.len() + unavailable_rows.len());
    for tool in native_tools {
        rows.push(match &tool.execution {
            NoemaToolExecution::LocalBuiltin => {
                format!("- builtin\t{}\t{}", tool.name, tool.description)
            }
            NoemaToolExecution::Mcp { .. } => {
                format!("- mcp\t{}\t{}", tool.name, tool.description)
            }
        });
    }
    rows.extend(unavailable_rows.iter().cloned());
    rows
}

fn store_tool_error(error: crate::StoreError) -> ToolContractError {
    ToolContractError::InvalidSchema(format!("failed to load model tools: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        McpCalibrationStatus, McpServerAuthStatus, McpServerHealthStatus, McpTransportKind,
        McpTrustClassification, NewMcpServer, NewMcpTool, NewToolCalibration,
        provider::{ProviderToolCapabilities, ProviderToolFallbackMode, ProviderToolSchemaDialect},
    };
    use serde_json::json;

    #[tokio::test]
    async fn native_provider_gets_builtin_and_calibrated_mcp_tools() {
        let store = crate::store::tests::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        seed_ready_mcp_tool(&store).await;

        let tools = build_model_tools(
            &store,
            true,
            ProviderToolCapabilities {
                native_tools: true,
                parallel_tool_calls: true,
                tool_choice: true,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                strict_schema: false,
                custom_tools: false,
                native_tool_results: true,
                fallback_mode: ProviderToolFallbackMode::NativeRequired,
            },
        )
        .await
        .expect("tools");

        let names = tools
            .native
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["search_memory", "update_own_name", "mcp.docs.read"]
        );
        assert!(tools.legacy_builtin_envelope_tools.is_empty());
    }

    #[tokio::test]
    async fn non_native_provider_gets_only_builtin_envelope_fallback() {
        let store = crate::store::tests::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        seed_ready_mcp_tool(&store).await;

        let tools = build_model_tools(
            &store,
            true,
            ProviderToolCapabilities {
                fallback_mode: ProviderToolFallbackMode::BuiltinOnlyEnvelope,
                ..ProviderToolCapabilities::default()
            },
        )
        .await
        .expect("tools");

        assert!(tools.native.is_empty());
        assert_eq!(
            tools.legacy_builtin_envelope_tools,
            vec!["search_memory".to_string(), "update_own_name".to_string()]
        );
        assert!(
            tools
                .unavailable_rows
                .iter()
                .all(|row| !row.contains("mcp.docs.read"))
        );
    }

    async fn seed_ready_mcp_tool(store: &crate::NoemaStore) {
        let server = store
            .create_mcp_server(NewMcpServer {
                mcp_server_id: "docs".to_string(),
                display_name: "Docs".to_string(),
                transport_kind: McpTransportKind::Stdio,
                safe_config: json!({}),
            })
            .await
            .expect("server");
        store
            .update_mcp_server_setup_status(
                &server.mcp_server_id,
                McpServerHealthStatus::Healthy,
                McpServerAuthStatus::Authenticated,
            )
            .await
            .expect("server state");
        store
            .upsert_discovered_mcp_tool(NewMcpTool {
                mcp_tool_id: "docs:read".to_string(),
                mcp_server_id: "docs".to_string(),
                name: "read".to_string(),
                description: Some("Read a document.".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {"document_id": {"type": "string"}},
                    "required": ["document_id"],
                    "additionalProperties": false
                }),
                output_schema: None,
                annotations: json!({}),
                metadata_fingerprint: "fp1".to_string(),
            })
            .await
            .expect("tool");
        store
            .save_tool_calibration(NewToolCalibration {
                calibration_id: "cal_docs_read".to_string(),
                mcp_tool_id: "docs:read".to_string(),
                read_classification: McpTrustClassification::Trusted,
                write_classification: McpTrustClassification::None,
                export_classification: McpTrustClassification::None,
                owner_extractors: Vec::new(),
                status: McpCalibrationStatus::Ready,
                reviewed_by: Some("human:local".to_string()),
                reviewed_metadata_fingerprint: Some("fp1".to_string()),
            })
            .await
            .expect("calibration");
    }
}
