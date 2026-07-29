//! Model-visible discovery and setup entrypoint for public hosted MCP services.

use std::collections::BTreeMap;

use noema_capabilities::{CapabilityError, CapabilityOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    CreateMcpServerCommand, LocalMcpService, McpOperations, McpSecretMaterial,
    McpServerSetupResult, McpSetupAuthPreference, McpSetupStatus, McpSetupTransportConfig,
    McpStreamableHttpSetupConfig,
    server_card::{McpServiceCardError, discover_mcp_service},
};

const MAX_SERVICE_URL_BYTES: usize = 4_096;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectServiceInput {
    service_url: String,
}

impl LocalMcpService {
    pub(crate) async fn connect_service_from_chat(
        &self,
        arguments: Value,
    ) -> Result<CapabilityOutput, CapabilityError> {
        let input: ConnectServiceInput =
            serde_json::from_value(arguments).map_err(|_| CapabilityError::InvalidArguments)?;
        if input.service_url.len() > MAX_SERVICE_URL_BYTES {
            return Err(CapabilityError::InvalidArguments);
        }
        let context = self
            .inner
            .request_context(self.inner.config.discovery_timeout);
        let discovered = match discover_mcp_service(&input.service_url, &context).await {
            Ok(Some(discovered)) => discovered,
            Ok(None) => {
                return Ok(CapabilityOutput::success(json!({
                    "status": "not_found",
                    "service_url": input.service_url,
                    "next_step": "Tell the human that the official website did not publish a supported MCP server card. Do not guess an endpoint. If appropriate, investigate the service's public HTTP API instead."
                })));
            }
            Err(error) => return Ok(discovery_failure_output(&input.service_url, error)),
        };
        let setup_input = json!({
            "displayName": discovered.display_name,
            "transportKind": "streamable_http",
            "stdio": null,
            "http": {
                "url": discovered.endpoint_url,
                "headers": {},
                "secretHeaders": {},
                "oauthClientCredentials": null
            }
        });
        let result = McpOperations::create_server(
            self,
            CreateMcpServerCommand {
                display_name: discovered.display_name.clone(),
                transport: McpSetupTransportConfig::StreamableHttp(McpStreamableHttpSetupConfig {
                    url: discovered.endpoint_url.clone(),
                    headers: BTreeMap::new(),
                }),
                secrets: McpSecretMaterial::default(),
                auth_preference: McpSetupAuthPreference::PromptIfAvailable,
            },
        )
        .await
        .map_err(map_setup_error)?;
        Ok(CapabilityOutput::success(json!({
            "status": setup_status_label(result.setup_status),
            "service_url": discovered.service_url,
            "server_card_url": discovered.server_card_url,
            "display_name": discovered.display_name,
            "description": discovered.description,
            "endpoint_url": discovered.endpoint_url,
            "setup_input": setup_input,
            "setup_result": setup_result_json(&result),
            "next_step": setup_next_step(result.setup_status)
        })))
    }
}

fn discovery_failure_output(service_url: &str, error: McpServiceCardError) -> CapabilityOutput {
    let (status, next_step) = match error {
        McpServiceCardError::InvalidInput | McpServiceCardError::Unsupported => (
            "invalid_card",
            "Tell the human that the official site did not publish a supported hosted MCP server card. Do not guess an endpoint.",
        ),
        McpServiceCardError::Unavailable => (
            "unavailable",
            "Tell the human that Noema could not check the service's MCP server card right now and that they can retry.",
        ),
    };
    CapabilityOutput::success(json!({
        "status": status,
        "service_url": service_url,
        "next_step": next_step
    }))
}

fn setup_result_json(result: &McpServerSetupResult) -> Value {
    json!({
        "setup_status": setup_status_label(result.setup_status),
        "discovery_status": result.discovery_status.map(|status| match status {
            crate::McpDiscoveryStatus::NeedsAuth => "needs_auth",
            crate::McpDiscoveryStatus::Discovered => "discovered",
            crate::McpDiscoveryStatus::Unavailable => "unavailable",
            crate::McpDiscoveryStatus::Malformed => "malformed",
        }),
        "discovered_tool_count": result.discovered_tool_count,
        "setup_error": result.issue.map(|issue| issue.to_string()),
        "auth": result.auth.as_ref().map(|auth| json!({
            "oauth_authorization_supported": auth.oauth_authorization_supported,
            "oauth_client_credentials_supported": auth.oauth_client_credentials_supported,
            "scopes": auth.scopes,
        })),
        "server": result.server.as_ref().map(|server| json!({
            "mcp_server_id": server.mcp_server_id,
            "display_name": server.display_name,
            "connection_revision": server.authority_generation,
            "policy_revision": server.policy_revision,
            "tool_count": server.tool_count,
        })),
    })
}

const fn setup_status_label(status: McpSetupStatus) -> &'static str {
    match status {
        McpSetupStatus::NeedsAuth => "needs_auth",
        McpSetupStatus::AuthenticationAvailable => "authentication_available",
        McpSetupStatus::ReadyForPolicy => "ready_for_policy",
        McpSetupStatus::Unavailable => "unavailable",
        McpSetupStatus::Malformed => "malformed",
    }
}

const fn setup_next_step(status: McpSetupStatus) -> &'static str {
    match status {
        McpSetupStatus::NeedsAuth
        | McpSetupStatus::AuthenticationAvailable
        | McpSetupStatus::ReadyForPolicy => {
            "Do not narrate setup status or send the human to Settings. The human-intervention surface now owns authentication and policy setup."
        }
        McpSetupStatus::Unavailable => {
            "Tell the human that Noema found the official MCP endpoint but could not connect to it, and that they can retry."
        }
        McpSetupStatus::Malformed => {
            "Tell the human that the official MCP endpoint returned unsupported tool metadata."
        }
    }
}

const fn map_setup_error(error: crate::McpOperationError) -> CapabilityError {
    match error {
        crate::McpOperationError::InvalidInput => CapabilityError::InvalidArguments,
        crate::McpOperationError::NotFound | crate::McpOperationError::Conflict => {
            CapabilityError::Failed
        }
        crate::McpOperationError::AuthenticationRequired
        | crate::McpOperationError::Unavailable
        | crate::McpOperationError::Cancelled
        | crate::McpOperationError::TimedOut
        | crate::McpOperationError::ShuttingDown => CapabilityError::Unavailable,
        crate::McpOperationError::MalformedResponse | crate::McpOperationError::Failed => {
            CapabilityError::Failed
        }
    }
}
