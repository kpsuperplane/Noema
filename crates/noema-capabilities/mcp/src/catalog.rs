#[cfg(any(feature = "transport", test))]
use std::sync::Arc;

use noema_capabilities::OperationToken;
#[cfg(any(feature = "transport", test))]
use noema_capabilities::{
    CapabilityAvailabilityNotice, CapabilityAvailabilityStatus, CapabilityBinding,
    CapabilityBindingSourceError, CapabilityCatalogBuilder, CapabilityCatalogResult,
    CapabilityConnectionPolicy, CapabilityDestination, CapabilityExecutionDecision,
    CapabilityScope, CapabilityServiceContext, CapabilityTarget, CapabilityToolBehavior,
    InvokerKey, RedactingPayloadSanitizer, ToolName, ToolSpec,
    resolve_capability_execution_decision,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "transport")]
use crate::McpRepositoryErrorKind;
#[cfg(any(feature = "transport", test))]
use crate::{
    McpControlPlaneServer, McpControlPlaneTool, McpServerAuthStatus, McpServerHealthStatus,
    McpToolPolicyRecord,
    eligibility::{
        mcp_tool_catalog_ineligibility, mcp_tool_ineligibility, prompt_safe_mcp_tool_description,
    },
    limits::bounded_provider_schema,
};

/// Invoker registry key used by MCP capability bindings.
pub(crate) const MCP_INVOKER_KEY: &str = "mcp";
pub(crate) const CONNECT_SERVICE_TOOL: &str = "mcp.connect_service";
const CONNECT_SERVICE_TOKEN: &str = "mcp-setup-v1:connect-service";
const MAX_SERVICE_URL_BYTES: usize = 4_096;

#[cfg(any(feature = "transport", test))]
fn connect_service_binding() -> Result<CapabilityBinding, CapabilityBindingSourceError> {
    let spec = ToolSpec::new(
        CONNECT_SERVICE_TOOL,
        concat!(
            "Discover and start chat-first setup for an official hosted MCP service. When the human asks to connect a service, first use web search to identify the service's official HTTPS website, then pass that website URL here. ",
            "Noema fetches the site's /.well-known/mcp.json server card, verifies the advertised Streamable HTTP endpoint, and starts connection discovery. Do not guess an MCP endpoint or pass a third-party directory, documentation mirror, API endpoint, token, cookie, or other credential."
        ),
        serde_json::json!({
            "type": "object",
            "properties": {
                "service_url": {
                    "type": "string",
                    "maxLength": MAX_SERVICE_URL_BYTES,
                    "description": "Official public website URL for the service, such as https://notion.com/."
                }
            },
            "required": ["service_url"],
            "additionalProperties": false
        }),
    )
    .map_err(|_| CapabilityBindingSourceError::Invalid)?;
    Ok(CapabilityBinding::new(
        spec,
        CapabilityTarget::new(
            InvokerKey::new(MCP_INVOKER_KEY),
            OperationToken::new(CONNECT_SERVICE_TOKEN),
        ),
        CapabilityToolBehavior {
            read_only: false,
            idempotent: false,
            destructive: false,
            open_world: true,
        },
        CapabilityExecutionDecision::ExecuteImmediately,
        CapabilityScope::Global,
        Arc::new(RedactingPayloadSanitizer),
    ))
}

#[cfg(feature = "transport")]
pub(crate) fn is_connect_service_invocation(operation: &ToolName, token: &OperationToken) -> bool {
    operation.as_str() == CONNECT_SERVICE_TOOL && token.as_str() == CONNECT_SERVICE_TOKEN
}

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
    server_policy_revision: u64,
    tool_policy_revision: u64,
}

impl McpOperationAuthority {
    #[cfg(any(feature = "transport", test))]
    fn capture(
        canonical_name: String,
        server: &McpControlPlaneServer,
        tool: &McpControlPlaneTool,
        policy: &McpToolPolicyRecord,
    ) -> Self {
        Self {
            canonical_name,
            server_id: server.server.mcp_server_id.clone(),
            authority_generation: server.server.authority_generation.clone(),
            tool_id: tool.tool.mcp_tool_id.clone(),
            metadata_fingerprint: tool.tool.metadata_fingerprint.clone(),
            server_policy_revision: server.server.policy_revision,
            tool_policy_revision: policy.policy_revision,
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

    #[cfg(feature = "transport")]
    pub(crate) fn canonical_name(&self) -> &str {
        &self.canonical_name
    }

    #[cfg(feature = "transport")]
    pub(crate) fn server_id(&self) -> &str {
        &self.server_id
    }

    #[cfg(feature = "transport")]
    pub(crate) fn tool_id(&self) -> &str {
        &self.tool_id
    }

    pub(crate) fn matches(
        &self,
        server: &crate::McpServerRecord,
        tool: &crate::McpToolRecord,
        policy: &McpToolPolicyRecord,
    ) -> bool {
        server.mcp_server_id == self.server_id
            && server.authority_generation == self.authority_generation
            && tool.mcp_server_id == self.server_id
            && tool.mcp_tool_id == self.tool_id
            && tool.metadata_fingerprint == self.metadata_fingerprint
            && server.policy_revision == self.server_policy_revision
            && policy.policy_revision == self.tool_policy_revision
            && policy.source_revision == self.metadata_fingerprint
    }
}

#[cfg(any(feature = "transport", test))]
pub(crate) fn catalog_from_servers(
    servers: &[McpControlPlaneServer],
) -> Result<CapabilityCatalogResult, CapabilityBindingSourceError> {
    let mut builder = CapabilityCatalogBuilder::new();
    builder
        .add(connect_service_binding()?)
        .map_err(|_| CapabilityBindingSourceError::Invalid)?;
    let mut availability_notices = Vec::new();
    for server in servers {
        let service_context = CapabilityServiceContext::new(
            server.server.display_name.clone(),
            server.server.service_description.clone(),
        )
        .map_err(|_| CapabilityBindingSourceError::Invalid)?;
        for tool in &server.tools {
            if mcp_tool_catalog_ineligibility(&server.server, &tool.tool, tool.policy.as_ref()) {
                continue;
            }
            let Some(policy) = tool.policy.as_ref() else {
                continue;
            };
            let canonical_name = format!("mcp.{}.{}", server.server.mcp_server_id, tool.tool.name);
            let name = ToolName::new(&canonical_name)
                .map_err(|_| CapabilityBindingSourceError::Invalid)?;
            let callable = !mcp_tool_ineligibility(&server.server, &tool.tool, Some(policy));
            if !callable {
                availability_notices.push(CapabilityAvailabilityNotice {
                    capability: Some(name.clone()),
                    status: if !server.server.enabled {
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
                    },
                });
            }
            let description =
                prompt_safe_mcp_tool_description(tool.tool.description.as_deref(), 96)
                    .unwrap_or_else(|| "MCP tool".to_string());
            let input_schema = bounded_provider_schema(&tool.tool.input_schema)
                .ok_or(CapabilityBindingSourceError::Invalid)?;
            let spec = ToolSpec::new(name.as_str(), description, input_schema)
                .map_err(|_| CapabilityBindingSourceError::Invalid)?;
            let authority = McpOperationAuthority::capture(canonical_name, server, tool, policy);
            let execution_decision = execution_decision(&server.server, policy);
            let destination = CapabilityDestination::new(
                "mcp",
                server.server.mcp_server_id.clone(),
                None::<String>,
                server.server.authority_generation.clone(),
            )
            .map_err(|_| CapabilityBindingSourceError::Invalid)?;
            builder
                .add(
                    CapabilityBinding::new(
                        spec,
                        CapabilityTarget::new(
                            InvokerKey::new(MCP_INVOKER_KEY),
                            authority.operation_token(),
                        ),
                        tool_behavior(policy),
                        execution_decision,
                        CapabilityScope::Global,
                        Arc::new(RedactingPayloadSanitizer),
                    )
                    .with_destination(destination)
                    .with_service_context(service_context.clone()),
                )
                .map_err(|_| CapabilityBindingSourceError::Invalid)?;
        }
    }
    Ok(CapabilityCatalogResult {
        snapshot: builder.build(),
        availability_notices,
    })
}

#[cfg(any(feature = "transport", test))]
pub(crate) fn tool_behavior(policy: &McpToolPolicyRecord) -> CapabilityToolBehavior {
    policy
        .behavior()
        .expect("catalog eligibility requires complete behavior")
}

#[cfg(any(feature = "transport", test))]
pub(crate) fn execution_decision(
    server: &crate::McpServerRecord,
    policy: &McpToolPolicyRecord,
) -> CapabilityExecutionDecision {
    resolve_capability_execution_decision(
        CapabilityConnectionPolicy {
            data_sharing: server
                .data_sharing_policy
                .expect("catalog eligibility requires provider policy"),
            unsafe_actions: server
                .unsafe_action_policy
                .expect("catalog eligibility requires provider policy"),
            revision: server.policy_revision,
        },
        tool_behavior(policy),
    )
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
    use noema_capabilities::{
        CapabilityError, CapabilityFuture, CapabilityInvocation, CapabilityInvoker,
        CapabilityOutput, CapabilityRegistryRouter, CapabilityRouter,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct CountingInvoker(AtomicUsize);

    impl CapabilityInvoker for CountingInvoker {
        fn invoke(
            &self,
            _invocation: CapabilityInvocation,
        ) -> CapabilityFuture<'_, Result<CapabilityOutput, CapabilityError>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(CapabilityOutput::success(serde_json::json!({"ok":true}))) })
        }
    }

    #[test]
    fn operation_token_contains_only_lookup_authority_and_round_trips() {
        let server = ready_server();
        let tool = &server.tools[0];
        let policy = tool.policy.as_ref().expect("policy");
        let authority =
            McpOperationAuthority::capture("mcp.mcp:docs.read".to_string(), &server, tool, policy);

        let token = authority.operation_token();
        let encoded = token.as_str();
        assert!(!encoded.contains("safe_config"));
        assert!(!encoded.contains("secret"));
        assert_eq!(
            McpOperationAuthority::from_operation_token(&token).expect("decode"),
            authority
        );
        assert!(authority.matches(&server.server, &tool.tool, policy));
    }

    #[test]
    fn catalog_always_advertises_chat_first_service_discovery() {
        let catalog = catalog_from_servers(&[]).expect("catalog");
        let binding = catalog
            .snapshot
            .resolve(CONNECT_SERVICE_TOOL)
            .expect("connect service binding");
        assert_eq!(binding.target().invoker_key().as_str(), MCP_INVOKER_KEY);
        assert_eq!(
            binding.spec().input_schema.as_value()["required"],
            serde_json::json!(["service_url"])
        );
        assert!(!binding.behavior().read_only);
        assert!(binding.behavior().open_world);
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

    #[test]
    fn safe_risky_sharing_and_approval_matrix_selects_the_execution_decision() {
        use crate::{McpDataSharingPolicy, McpUnsafeActionPolicy};

        let joined = ready_server();
        let base_policy = joined.tools[0].policy.as_ref().expect("policy");
        for unsafe_actions in [
            McpUnsafeActionPolicy::AlwaysAsk,
            McpUnsafeActionPolicy::ReviewerMayApprove,
            McpUnsafeActionPolicy::NeverAsk,
        ] {
            let mut server = joined.server.clone();
            server.unsafe_action_policy = Some(unsafe_actions);
            server.data_sharing_policy = Some(McpDataSharingPolicy::AllowAutomatically);
            assert_eq!(
                execution_decision(&server, base_policy),
                CapabilityExecutionDecision::ExecuteImmediately
            );

            let mut risky = base_policy.clone();
            risky.read_only.value = Some(false);
            risky.destructive.value = Some(true);
            let risky_decision = match unsafe_actions {
                McpUnsafeActionPolicy::AlwaysAsk => CapabilityExecutionDecision::HumanReview,
                McpUnsafeActionPolicy::ReviewerMayApprove => CapabilityExecutionDecision::LlmReview,
                McpUnsafeActionPolicy::NeverAsk => CapabilityExecutionDecision::ExecuteImmediately,
            };
            assert_eq!(execution_decision(&server, &risky), risky_decision);

            let mut contradictory = base_policy.clone();
            contradictory.destructive.value = Some(true);
            assert_eq!(
                execution_decision(&server, &contradictory),
                CapabilityExecutionDecision::ExecuteImmediately
            );
        }

        for risky in [false, true] {
            let mut server = joined.server.clone();
            server.data_sharing_policy = Some(McpDataSharingPolicy::ReviewEveryCall);
            let mut policy = base_policy.clone();
            if risky {
                policy.read_only.value = Some(false);
                policy.open_world.value = Some(true);
            }
            server.unsafe_action_policy = Some(McpUnsafeActionPolicy::AlwaysAsk);
            assert_eq!(
                execution_decision(&server, &policy),
                CapabilityExecutionDecision::HumanReview
            );
            server.unsafe_action_policy = Some(McpUnsafeActionPolicy::ReviewerMayApprove);
            assert_eq!(
                execution_decision(&server, &policy),
                CapabilityExecutionDecision::LlmReview
            );
        }
    }

    #[tokio::test]
    async fn safe_additive_closed_world_mutation_reaches_the_invoker() {
        let mut server = ready_server();
        let policy = server.tools[0].policy.as_mut().expect("policy");
        policy.read_only.value = Some(false);
        policy.destructive.value = Some(false);
        policy.open_world.value = Some(false);
        let catalog = catalog_from_servers(&[server]).expect("catalog");
        let invoker = Arc::new(CountingInvoker::default());
        let router = CapabilityRegistryRouter::new([(
            InvokerKey::new(MCP_INVOKER_KEY),
            invoker.clone() as Arc<dyn CapabilityInvoker>,
        )])
        .expect("router");

        router
            .dispatch(
                catalog.snapshot,
                "mcp.mcp:docs.read".to_string(),
                serde_json::json!({}),
            )
            .await
            .expect("safe mutation executes immediately");
        assert_eq!(invoker.0.load(Ordering::SeqCst), 1);
    }
}
