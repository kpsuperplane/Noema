#[cfg(any(feature = "transport", test))]
use std::sync::Arc;

use noema_capabilities::OperationToken;
#[cfg(any(feature = "transport", test))]
use noema_capabilities::{
    CapabilityAccess, CapabilityAvailabilityNotice, CapabilityAvailabilityStatus,
    CapabilityBinding, CapabilityBindingSourceError, CapabilityCatalogBuilder,
    CapabilityCatalogResult, CapabilityEffect, CapabilityScope, CapabilityTarget, InvokerKey,
    OmitPayloadSanitizer, ToolName, ToolSpec,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "transport")]
use crate::McpRepositoryErrorKind;
use crate::ToolCalibrationRecord;
#[cfg(any(feature = "transport", test))]
use crate::{
    McpControlPlaneServer, McpControlPlaneTool, McpServerAuthStatus, McpServerHealthStatus,
    eligibility::{
        mcp_tool_catalog_ineligibility, mcp_tool_ineligibility, prompt_safe_mcp_tool_description,
    },
    limits::bounded_provider_schema,
};

/// Invoker registry key used by MCP capability bindings.
pub(crate) const MCP_INVOKER_KEY: &str = "mcp";

/// Immutable lookup authority captured when an MCP binding is advertised.
///
/// The token contains identifiers and reviewed revisions only. Connection
/// configuration and policy are re-read from the repository at invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct McpOperationAuthority {
    canonical_name: String,
    server_id: String,
    authority_generation: String,
    tool_id: String,
    metadata_fingerprint: String,
    calibration_id: String,
    reviewed_metadata_fingerprint: String,
}

impl McpOperationAuthority {
    #[cfg(any(feature = "transport", test))]
    fn capture(
        canonical_name: String,
        server: &McpControlPlaneServer,
        tool: &McpControlPlaneTool,
        calibration: &ToolCalibrationRecord,
    ) -> Self {
        Self {
            canonical_name,
            server_id: server.server.mcp_server_id.clone(),
            authority_generation: server.server.authority_generation.clone(),
            tool_id: tool.tool.mcp_tool_id.clone(),
            metadata_fingerprint: tool.tool.metadata_fingerprint.clone(),
            calibration_id: calibration.calibration_id.clone(),
            reviewed_metadata_fingerprint: calibration
                .reviewed_metadata_fingerprint
                .clone()
                .expect("catalog eligibility requires a reviewed fingerprint"),
        }
    }

    fn operation_token(&self) -> OperationToken {
        OperationToken::new(
            serde_json::to_string(self).expect("MCP operation authority is serializable"),
        )
    }

    pub(crate) fn from_operation_token(
        token: &OperationToken,
    ) -> Result<Self, noema_capabilities::CapabilityError> {
        serde_json::from_str(token.as_str())
            .map_err(|_| noema_capabilities::CapabilityError::UnknownOperation)
    }

    pub(crate) fn canonical_name(&self) -> &str {
        &self.canonical_name
    }

    pub(crate) fn server_id(&self) -> &str {
        &self.server_id
    }

    pub(crate) fn tool_id(&self) -> &str {
        &self.tool_id
    }

    pub(crate) fn matches(
        &self,
        server: &crate::McpServerRecord,
        tool: &crate::McpToolRecord,
        calibration: &ToolCalibrationRecord,
    ) -> bool {
        server.mcp_server_id == self.server_id
            && server.authority_generation == self.authority_generation
            && tool.mcp_server_id == self.server_id
            && tool.mcp_tool_id == self.tool_id
            && tool.metadata_fingerprint == self.metadata_fingerprint
            && calibration.calibration_id == self.calibration_id
            && calibration.reviewed_metadata_fingerprint.as_deref()
                == Some(self.reviewed_metadata_fingerprint.as_str())
    }
}

#[cfg(any(feature = "transport", test))]
pub(crate) fn catalog_from_servers(
    servers: &[McpControlPlaneServer],
) -> Result<CapabilityCatalogResult, CapabilityBindingSourceError> {
    let mut builder = CapabilityCatalogBuilder::new();
    let mut availability_notices = Vec::new();
    for server in servers {
        for tool in &server.tools {
            if mcp_tool_catalog_ineligibility(&tool.tool, tool.calibration.as_ref()).is_some() {
                continue;
            }
            let Some(calibration) = tool.calibration.as_ref() else {
                continue;
            };
            let canonical_name = format!("mcp.{}.{}", server.server.mcp_server_id, tool.tool.name);
            let name = ToolName::new(&canonical_name)
                .map_err(|_| CapabilityBindingSourceError::Invalid)?;
            let callable =
                mcp_tool_ineligibility(&server.server, &tool.tool, Some(calibration)).is_none();
            if !callable {
                availability_notices.push(CapabilityAvailabilityNotice {
                    capability: Some(name.clone()),
                    status: availability_status(server),
                });
            }
            let description =
                prompt_safe_mcp_tool_description(tool.tool.description.as_deref(), 96)
                    .unwrap_or_else(|| "MCP tool".to_string());
            let input_schema = bounded_provider_schema(&tool.tool.input_schema)
                .ok_or(CapabilityBindingSourceError::Invalid)?;
            let spec = ToolSpec::new(name.as_str(), description, input_schema)
                .map_err(|_| CapabilityBindingSourceError::Invalid)?;
            let authority =
                McpOperationAuthority::capture(canonical_name, server, tool, calibration);
            builder
                .add(CapabilityBinding::new(
                    spec,
                    CapabilityTarget::new(
                        InvokerKey::new(MCP_INVOKER_KEY),
                        authority.operation_token(),
                    ),
                    CapabilityAccess {
                        effect: CapabilityEffect::ReadOnly,
                        scope: CapabilityScope::Global,
                    },
                    Arc::new(OmitPayloadSanitizer),
                ))
                .map_err(|_| CapabilityBindingSourceError::Invalid)?;
        }
    }
    Ok(CapabilityCatalogResult {
        snapshot: builder.build(),
        availability_notices,
    })
}

#[cfg(any(feature = "transport", test))]
fn availability_status(server: &McpControlPlaneServer) -> CapabilityAvailabilityStatus {
    if !server.server.enabled {
        CapabilityAvailabilityStatus::Disabled
    } else if !matches!(
        server.server.auth_status,
        McpServerAuthStatus::None | McpServerAuthStatus::Authenticated
    ) {
        CapabilityAvailabilityStatus::AuthenticationRequired
    } else if server.server.health_status != McpServerHealthStatus::Healthy {
        CapabilityAvailabilityStatus::Unavailable
    } else {
        CapabilityAvailabilityStatus::Disabled
    }
}

#[cfg(feature = "transport")]
pub(crate) fn map_repository_error(
    error: crate::McpRepositoryError,
) -> CapabilityBindingSourceError {
    match error.kind() {
        McpRepositoryErrorKind::Unavailable => CapabilityBindingSourceError::Unavailable,
        McpRepositoryErrorKind::NotFound
        | McpRepositoryErrorKind::Conflict
        | McpRepositoryErrorKind::Invariant => CapabilityBindingSourceError::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixture::ready_server;

    #[test]
    fn operation_token_contains_only_lookup_authority_and_round_trips() {
        let server = ready_server();
        let tool = &server.tools[0];
        let calibration = tool.calibration.as_ref().expect("calibration");
        let authority = McpOperationAuthority::capture(
            "mcp.mcp:docs.read".to_string(),
            &server,
            tool,
            calibration,
        );

        let token = authority.operation_token();
        let encoded = token.as_str();
        assert!(!encoded.contains("safe_config"));
        assert!(!encoded.contains("secret"));
        assert_eq!(
            McpOperationAuthority::from_operation_token(&token).expect("decode"),
            authority
        );
        assert!(authority.matches(&server.server, &tool.tool, calibration));
    }

    #[test]
    fn catalog_preserves_the_exact_bounded_schema_covered_by_human_review() {
        let mut server = ready_server();
        server.tools[0].tool.input_schema = serde_json::json!({
            "type": "object",
            "$defs": {
                "documentId": {
                    "type": "string",
                    "description": "The durable document identifier"
                }
            },
            "properties": {
                "document_id": {"$ref": "#/$defs/documentId"}
            },
            "required": ["document_id"]
        });
        let expected = server.tools[0].tool.input_schema.clone();

        let catalog = catalog_from_servers(&[server]).expect("catalog");
        let binding = catalog
            .snapshot
            .resolve("mcp.mcp:docs.read")
            .expect("binding");
        assert_eq!(binding.spec().input_schema.as_value(), &expected);
    }
}
