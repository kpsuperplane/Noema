use std::{fmt, sync::Arc};

use noema_capabilities::{
    CapabilityAccess, CapabilityAvailabilityNotice, CapabilityAvailabilityStatus,
    CapabilityBinding, CapabilityBindingSource, CapabilityBindingSourceError,
    CapabilityBindingSourceHandle, CapabilityCatalogBuilder, CapabilityCatalogResult,
    CapabilityEffect, CapabilityFuture, CapabilityScope, CapabilityTarget, InvokerKey,
    OmitPayloadSanitizer, OperationToken, ToolName, ToolSpec,
};
use serde::{Deserialize, Serialize};

use crate::{
    McpControlPlaneServer, McpControlPlaneTool, McpRepositoryErrorKind, McpRepositoryHandle,
    McpServerAuthStatus, McpServerHealthStatus, ToolCalibrationRecord,
    limits::bounded_provider_schema, mcp_tool_catalog_ineligibility, mcp_tool_ineligibility,
    prompt_safe_mcp_tool_description,
};

/// Invoker registry key used by MCP capability bindings.
pub const MCP_INVOKER_KEY: &str = "mcp";

/// Immutable lookup authority captured when an MCP binding is advertised.
///
/// The token contains identifiers and reviewed revisions only. Connection
/// configuration and policy are re-read from the repository at invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpOperationAuthority {
    canonical_name: String,
    server_id: String,
    authority_generation: String,
    tool_id: String,
    metadata_fingerprint: String,
    calibration_id: String,
    reviewed_metadata_fingerprint: String,
}

impl McpOperationAuthority {
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

    /// Encode this child-owned authority into the parent capability token.
    #[must_use]
    pub fn operation_token(&self) -> OperationToken {
        OperationToken::new(
            serde_json::to_string(self).expect("MCP operation authority is serializable"),
        )
    }

    /// Decode child-owned authority from a capability token.
    ///
    /// # Errors
    ///
    /// Returns `UnknownOperation` when the token is malformed.
    pub fn from_operation_token(
        token: &OperationToken,
    ) -> Result<Self, noema_capabilities::CapabilityError> {
        serde_json::from_str(token.as_str())
            .map_err(|_| noema_capabilities::CapabilityError::UnknownOperation)
    }

    /// Canonical name captured in the provider-visible catalog.
    #[must_use]
    pub fn canonical_name(&self) -> &str {
        &self.canonical_name
    }

    /// Durable server id.
    #[must_use]
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Durable tool id.
    #[must_use]
    pub fn tool_id(&self) -> &str {
        &self.tool_id
    }

    /// Whether a joined current snapshot is exactly the authorized revision.
    #[must_use]
    pub fn matches(
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

/// MCP catalog projection backed by one repository snapshot.
#[derive(Clone)]
pub struct McpBindingSource {
    repository: McpRepositoryHandle,
}

impl McpBindingSource {
    /// Construct a catalog source over the injected repository.
    #[must_use]
    pub fn new(repository: McpRepositoryHandle) -> Self {
        Self { repository }
    }

    /// Return this source behind the parent object-safe handle.
    #[must_use]
    pub fn handle(self) -> CapabilityBindingSourceHandle {
        Arc::new(self)
    }
}

impl fmt::Debug for McpBindingSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpBindingSource")
            .field("repository", &"[CONFIGURED]")
            .finish()
    }
}

impl CapabilityBindingSource for McpBindingSource {
    fn catalog(
        &self,
    ) -> CapabilityFuture<'_, Result<CapabilityCatalogResult, CapabilityBindingSourceError>> {
        Box::pin(async move {
            let servers = self
                .repository
                .control_plane_catalog()
                .await
                .map_err(map_repository_error)?;
            catalog_from_servers(&servers)
        })
    }
}

fn catalog_from_servers(
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

fn map_repository_error(error: crate::McpRepositoryError) -> CapabilityBindingSourceError {
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
    use crate::{
        McpCalibrationStatus, McpServerRecord, McpToolRecord, McpTransportKind,
        McpTrustClassification,
    };

    #[test]
    fn operation_token_contains_only_lookup_authority_and_round_trips() {
        let server = fixture();
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
        let mut server = fixture();
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

    fn fixture() -> McpControlPlaneServer {
        let tool = McpToolRecord {
            mcp_tool_id: "mcp_tool:docs:read".to_string(),
            mcp_server_id: "mcp:docs".to_string(),
            name: "read".to_string(),
            description: Some("Read documents".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            annotations: serde_json::json!({}),
            metadata_fingerprint: "fingerprint".to_string(),
            discovered_at: "now".to_string(),
        };
        McpControlPlaneServer {
            server: McpServerRecord {
                mcp_server_id: "mcp:docs".to_string(),
                display_name: "Docs".to_string(),
                transport_kind: McpTransportKind::Stdio,
                safe_config: serde_json::json!({"command": "docs"}),
                enabled: true,
                health_status: McpServerHealthStatus::Healthy,
                auth_status: McpServerAuthStatus::None,
                tool_count: 1,
                authority_generation: "generation".to_string(),
            },
            tools: vec![McpControlPlaneTool {
                tool,
                calibration: Some(ToolCalibrationRecord {
                    calibration_id: "calibration".to_string(),
                    mcp_tool_id: "mcp_tool:docs:read".to_string(),
                    read_classification: McpTrustClassification::Trusted,
                    write_classification: McpTrustClassification::None,
                    export_classification: McpTrustClassification::None,
                    status: McpCalibrationStatus::Ready,
                    reviewed_by: Some("human:local".to_string()),
                    reviewed_metadata_fingerprint: Some("fingerprint".to_string()),
                }),
            }],
        }
    }
}
