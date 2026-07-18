use std::collections::BTreeMap;

use async_graphql::{Json, Result};
use noema_capabilities_mcp::{
    CreateMcpServerCommand, McpCalibrationStatus, McpOAuthClientCredentials, McpSecretMaterial,
    McpSetupTransportConfig, McpStdioSetupConfig, McpStreamableHttpSetupConfig, McpTransportKind,
    McpTrustClassification, NewToolCalibration,
};
use serde_json::Value;

use super::{
    GraphqlCreateMcpServerInput, GraphqlMcpOAuthClientCredentialsInput,
    GraphqlSaveToolCalibrationInput,
};
use crate::graphql::errors::graphql_error;

pub(super) fn parse_save_tool_calibration_input(
    input: GraphqlSaveToolCalibrationInput,
) -> Result<NewToolCalibration> {
    let read_classification =
        parse_graphql_trust_classification(&input.read_classification, "readClassification")?;
    let write_classification =
        parse_graphql_trust_classification(&input.write_classification, "writeClassification")?;
    let export_classification =
        parse_graphql_trust_classification(&input.export_classification, "exportClassification")?;
    let status = parse_graphql_calibration_status(&input.status)?;
    Ok(NewToolCalibration {
        calibration_id: input.calibration_id,
        mcp_tool_id: input.mcp_tool_id,
        read_classification,
        write_classification,
        export_classification,
        status,
        reviewed_by: input.reviewed_by,
        reviewed_metadata_fingerprint: input.reviewed_metadata_fingerprint,
    })
}

fn parse_graphql_trust_classification(
    value: &str,
    field_name: &'static str,
) -> Result<McpTrustClassification> {
    match value {
        "none" => Ok(McpTrustClassification::None),
        "trusted" => Ok(McpTrustClassification::Trusted),
        "untrusted" => Ok(McpTrustClassification::Untrusted),
        "mixed" => Ok(McpTrustClassification::Mixed),
        _ => Err(graphql_error(format!(
            "invalid {field_name}: expected one of none, trusted, untrusted, mixed"
        ))),
    }
}

pub(super) fn parse_create_mcp_server_input(
    input: GraphqlCreateMcpServerInput,
) -> Result<CreateMcpServerCommand> {
    let transport_kind = parse_graphql_transport_kind(&input.transport_kind)?;
    match transport_kind {
        McpTransportKind::Stdio => {
            if input.http.is_some() {
                return Err(graphql_error(
                    "invalid MCP setup input: http cannot be set for stdio transport",
                ));
            }
            let stdio = input.stdio.ok_or_else(|| {
                graphql_error("invalid MCP setup input: stdio config is required")
            })?;
            let env = json_string_map(stdio.env, "env")?;
            let secret_env = json_string_map(stdio.secret_env, "secretEnv")?;
            Ok(CreateMcpServerCommand {
                display_name: input.display_name,
                transport: McpSetupTransportConfig::Stdio(McpStdioSetupConfig {
                    command: stdio.command,
                    args: stdio.args,
                    cwd: stdio.cwd,
                    env,
                }),
                secrets: McpSecretMaterial {
                    env: secret_env,
                    ..McpSecretMaterial::default()
                },
            })
        }
        McpTransportKind::StreamableHttp => {
            if input.stdio.is_some() {
                return Err(graphql_error(
                    "invalid MCP setup input: stdio cannot be set for HTTP transport",
                ));
            }
            let http = input
                .http
                .ok_or_else(|| graphql_error("invalid MCP setup input: http config is required"))?;
            let headers = json_string_map(http.headers, "headers")?;
            let secret_headers = json_string_map(http.secret_headers, "secretHeaders")?;
            let oauth_client_credentials =
                parse_oauth_client_credentials(http.oauth_client_credentials)?;
            Ok(CreateMcpServerCommand {
                display_name: input.display_name,
                transport: McpSetupTransportConfig::StreamableHttp(McpStreamableHttpSetupConfig {
                    url: http.url,
                    headers,
                }),
                secrets: McpSecretMaterial {
                    headers: secret_headers,
                    oauth_client_credentials,
                    ..McpSecretMaterial::default()
                },
            })
        }
    }
}

pub(super) fn parse_oauth_client_credentials(
    input: Option<GraphqlMcpOAuthClientCredentialsInput>,
) -> Result<Option<McpOAuthClientCredentials>> {
    let Some(input) = input else {
        return Ok(None);
    };
    let client_id = input.client_id.trim();
    let client_secret = input.client_secret.trim();
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(graphql_error(
            "invalid MCP OAuth client credentials: clientId and clientSecret are required",
        ));
    }
    Ok(Some(McpOAuthClientCredentials {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        scopes: input
            .scopes
            .into_iter()
            .map(|scope| scope.trim().to_string())
            .filter(|scope| !scope.is_empty())
            .collect(),
    }))
}

fn parse_graphql_transport_kind(value: &str) -> Result<McpTransportKind> {
    match value {
        "stdio" => Ok(McpTransportKind::Stdio),
        "streamable_http" => Ok(McpTransportKind::StreamableHttp),
        _ => Err(graphql_error(
            "invalid transportKind: expected one of stdio, streamable_http",
        )),
    }
}

pub(super) fn json_string_map(
    value: Option<Json<Value>>,
    field_name: &'static str,
) -> Result<BTreeMap<String, String>> {
    let Some(Json(value)) = value else {
        return Ok(BTreeMap::new());
    };
    let object = value.as_object().ok_or_else(|| {
        graphql_error(format!(
            "invalid MCP {field_name} map: expected object with string values"
        ))
    })?;
    object
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|string| (key.clone(), string.to_string()))
                .ok_or_else(|| {
                    graphql_error(format!(
                        "invalid MCP {field_name} map: expected object with string values"
                    ))
                })
        })
        .collect()
}

fn parse_graphql_calibration_status(value: &str) -> Result<McpCalibrationStatus> {
    match value {
        "needs_review" => Ok(McpCalibrationStatus::NeedsReview),
        "blocked_unresolved_ownership" => Ok(McpCalibrationStatus::BlockedUnresolvedOwnership),
        "ready" => Ok(McpCalibrationStatus::Ready),
        "disabled" => Ok(McpCalibrationStatus::Disabled),
        _ => Err(graphql_error(
            "invalid status: expected one of needs_review, blocked_unresolved_ownership, ready, disabled",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_boundary_enums() {
        let error = parse_graphql_transport_kind("sse").expect_err("SSE must be rejected");

        assert!(error.message.contains("stdio, streamable_http"));

        let error = parse_save_tool_calibration_input(GraphqlSaveToolCalibrationInput {
            calibration_id: "calibration:read".to_string(),
            mcp_tool_id: "tool:read".to_string(),
            read_classification: "Mixed".to_string(),
            write_classification: "none".to_string(),
            export_classification: "none".to_string(),
            status: "blocked_unresolved_ownership".to_string(),
            reviewed_by: None,
            reviewed_metadata_fingerprint: None,
        })
        .expect_err("GraphQL enums use canonical lower-case storage values");
        assert!(error.message.contains("invalid readClassification"));
    }
}
