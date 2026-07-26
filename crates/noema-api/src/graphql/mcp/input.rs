use std::collections::BTreeMap;

use async_graphql::{Json, Result};
use noema_capabilities_mcp::{
    CreateMcpServerCommand, McpDataSharingPolicy, McpOAuthClientCredentials, McpSecretMaterial,
    McpSetupTransportConfig, McpStdioSetupConfig, McpStreamableHttpSetupConfig, McpTransportKind,
    McpUnsafeActionPolicy, validate_provider_policy,
};
use serde_json::Value;

use super::{GraphqlCreateMcpServerInput, GraphqlMcpOAuthClientCredentialsInput};
use crate::graphql::errors::graphql_error;

pub(super) fn parse_provider_policy_input(
    data_sharing: &str,
    unsafe_actions: &str,
) -> Result<(McpDataSharingPolicy, McpUnsafeActionPolicy)> {
    let data_sharing = data_sharing
        .parse::<McpDataSharingPolicy>()
        .map_err(|_| graphql_error("invalid MCP data sharing policy"))?;
    let unsafe_actions = unsafe_actions
        .parse::<McpUnsafeActionPolicy>()
        .map_err(|_| graphql_error("invalid MCP unsafe action policy"))?;
    validate_provider_policy(data_sharing, unsafe_actions)
        .map_err(|_| graphql_error("review_every_call cannot be combined with never_ask"))?;
    Ok((data_sharing, unsafe_actions))
}

pub(super) fn parse_create_mcp_server_input(
    input: GraphqlCreateMcpServerInput,
) -> Result<CreateMcpServerCommand> {
    let transport_kind = parse_graphql_transport_kind(&input.transport_kind)?;
    let auth_preference = input.auth_preference.map(Into::into).unwrap_or_default();
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
                auth_preference,
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
                auth_preference,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_boundary_enums() {
        let error = parse_graphql_transport_kind("sse").expect_err("SSE must be rejected");

        assert!(error.message.contains("stdio, streamable_http"));

        let error = parse_provider_policy_input("review_every_call", "never_ask")
            .expect_err("incompatible provider policies must be rejected at the API boundary");
        assert!(error.message.contains("cannot be combined"));
    }
}
