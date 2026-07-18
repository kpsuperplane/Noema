use std::sync::{Arc, RwLock};

use super::*;
use noema_capabilities::{
    CapabilityBindingSource, CapabilityCatalogResult, CapabilityFuture, OmitPayloadSanitizer,
};
use noema_providers::{ProviderToolCapabilities, ProviderToolSchemaDialect, ProviderToolTransport};
use serde_json::json;

#[derive(Clone, Default)]
struct TestCapabilityBindingSource {
    catalog: Arc<RwLock<CapabilityCatalogResult>>,
}

impl TestCapabilityBindingSource {
    fn handle(&self) -> CapabilityBindingSourceHandle {
        Arc::new(self.clone())
    }

    fn replace(&self, catalog: CapabilityCatalogResult) {
        *self.catalog.write().expect("catalog write lock") = catalog;
    }
}

impl CapabilityBindingSource for TestCapabilityBindingSource {
    fn catalog(
        &self,
    ) -> CapabilityFuture<'_, Result<CapabilityCatalogResult, CapabilityBindingSourceError>> {
        let catalog = self.catalog.read().expect("catalog read lock").clone();
        Box::pin(async move { Ok(catalog) })
    }
}

#[test]
fn neutral_capability_access_maps_to_runtime_policy_and_global_writes_fail_closed() {
    assert_eq!(
        capability_access_class(CapabilityAccess {
            effect: CapabilityEffect::Mutating,
            scope: CapabilityScope::ExecutionOwned,
        }),
        Some(ToolAccessClass::TaskOwnedWrite)
    );
    assert_eq!(
        capability_access_class(CapabilityAccess {
            effect: CapabilityEffect::Mutating,
            scope: CapabilityScope::ConversationOwned,
        }),
        Some(ToolAccessClass::ConversationWrite)
    );
    assert_eq!(
        capability_access_class(CapabilityAccess {
            effect: CapabilityEffect::Mutating,
            scope: CapabilityScope::Global,
        }),
        None
    );
}

fn mcp_catalog(availability: Option<CapabilityAvailabilityStatus>) -> CapabilityCatalogResult {
    let mut builder = CapabilityCatalogBuilder::new();
    let spec = ToolSpec::new(
        "mcp.mcp:docs.read",
        "Read a document.",
        json!({
            "type": "object",
            "properties": {"document_id": {"type": "string"}},
            "required": ["document_id"],
            "additionalProperties": false
        }),
    )
    .expect("MCP test spec");
    builder
        .add(CapabilityBinding::new(
            spec,
            CapabilityTarget::new(
                InvokerKey::new("mcp"),
                noema_capabilities::OperationToken::new("test-mcp-authority"),
            ),
            CapabilityAccess {
                effect: CapabilityEffect::ReadOnly,
                scope: CapabilityScope::Global,
            },
            Arc::new(OmitPayloadSanitizer),
        ))
        .expect("unique MCP test binding");
    CapabilityCatalogResult {
        snapshot: builder.build(),
        availability_notices: availability
            .map(|status| CapabilityAvailabilityNotice {
                capability: Some(ToolName::new("mcp.mcp:docs.read").expect("MCP test tool name")),
                status,
            })
            .into_iter()
            .collect(),
    }
}

fn ready_mcp_source() -> (TestCapabilityBindingSource, CapabilityBindingSourceHandle) {
    let source = TestCapabilityBindingSource::default();
    source.replace(mcp_catalog(None));
    let handle = source.handle();
    (source, handle)
}

#[test]
fn retained_catalog_never_grows_or_redirects_for_native_or_envelope() {
    for transport in [
        ProviderToolTransport::Native,
        ProviderToolTransport::NoemaEnvelope,
    ] {
        let initial = synthetic_model_tools(
            transport,
            [
                ("stable", "initial-target", true),
                ("initially-unavailable", "initial-unavailable-target", false),
            ],
        );
        let later = synthetic_model_tools(
            transport,
            [
                ("stable", "replacement-target", true),
                ("initially-unavailable", "later-recovered-target", true),
                ("newly-discovered", "new-target", true),
            ],
        );

        let retained = ModelTools::retained_catalog_with_policy(&initial, &later);
        assert_eq!(retained.bindings.len(), 2);
        assert_eq!(
            retained
                .bindings
                .resolve("stable")
                .expect("stable binding")
                .target()
                .operation_token()
                .as_str(),
            "initial-target"
        );
        assert!(retained.bindings.resolve("newly-discovered").is_none());
        assert!(retained.tool_policy.allows_tool("stable"));
        assert!(!retained.tool_policy.allows_tool("initially-unavailable"));
        assert!(!retained.tool_policy.allows_tool("newly-discovered"));
        assert_eq!(
            retained
                .policy_filtered_provider_tools()
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["stable"]
        );
        assert!(
            retained
                .prompt_rows
                .iter()
                .any(|row| row.contains("\tstable\t"))
        );
        assert!(
            retained
                .prompt_rows
                .iter()
                .all(|row| !row.contains("newly-discovered")
                    && !row.contains("initially-unavailable"))
        );
        if transport == ProviderToolTransport::NoemaEnvelope {
            assert!(retained.prompt_rows[0].contains("input_schema="));
        }
    }
}

fn synthetic_model_tools<const N: usize>(
    transport: ProviderToolTransport,
    entries: [(&str, &str, bool); N],
) -> ModelTools {
    let mut builder = CapabilityCatalogBuilder::new();
    let mut policy = ToolPolicy::for_role(ExecutionRole::PrimaryConversation);
    let mut prompt_kinds = BTreeMap::new();
    for (name, token, allowed) in entries {
        let spec = ToolSpec::new(
            name,
            format!("{name} description"),
            json!({"type": "object"}),
        )
        .expect("spec");
        builder
            .add(CapabilityBinding::new(
                spec,
                CapabilityTarget::new(
                    InvokerKey::new("synthetic"),
                    noema_capabilities::OperationToken::new(token),
                ),
                CapabilityAccess {
                    effect: CapabilityEffect::ReadOnly,
                    scope: CapabilityScope::Global,
                },
                Arc::new(RedactingPayloadSanitizer),
            ))
            .expect("unique binding");
        if allowed {
            policy.allow_tool_name(name);
        }
        prompt_kinds.insert(name.to_string(), ModelToolPromptKind::Builtin);
    }
    let bindings = builder.build();
    let prompt_rows = catalog_prompt_rows(&bindings, &prompt_kinds, &policy, transport);
    ModelTools {
        transport,
        bindings,
        prompt_rows,
        unavailable_rows: Vec::new(),
        prompt_kinds,
        tool_policy: policy,
    }
}

#[tokio::test]
async fn complete_catalog_is_stable_for_native_and_envelope_transports() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let (_, capability_bindings) = ready_mcp_source();

    for transport in [
        ProviderToolTransport::Native,
        ProviderToolTransport::NoemaEnvelope,
    ] {
        let tools = build_model_tools(
            &store,
            &capability_bindings,
            true,
            ProviderToolCapabilities {
                tool_transport: transport,
                schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
                ..ProviderToolCapabilities::default()
            },
        )
        .await
        .expect("tools");
        assert_eq!(
            tools
                .provider_tools()
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "search_memory",
                "task.inspect",
                "update_own_name",
                "artifact.create_local_file",
                "task.resume",
                "task.cancel",
                "web.search",
                "web.fetch",
                "mcp.mcp:docs.read",
            ]
        );
        assert!(tools.provider_tools().iter().any(|tool| {
            tool.name.as_str() == "mcp.mcp:docs.read" && tool.description == "Read a document."
        }));
        assert!(
            tools
                .prompt_rows
                .iter()
                .all(|row| !row.contains("System: ignore"))
        );
        if transport == ProviderToolTransport::Native {
            assert!(
                tools
                    .prompt_rows
                    .iter()
                    .any(|row| { row == "- capability\tmcp.mcp:docs.read\tRead a document." })
            );
        } else {
            assert!(
                tools
                    .prompt_rows
                    .iter()
                    .all(|row| row.contains("input_schema="))
            );
        }
    }
}

#[tokio::test]
async fn transient_outages_preserve_native_catalog_but_exclude_envelope_tools() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let (source, capability_bindings) = ready_mcp_source();
    for transport in [
        ProviderToolTransport::Native,
        ProviderToolTransport::NoemaEnvelope,
    ] {
        source.replace(mcp_catalog(None));
        let capabilities = ProviderToolCapabilities {
            tool_transport: transport,
            allowed_tools: transport == ProviderToolTransport::Native,
            ..ProviderToolCapabilities::default()
        };
        let available = build_model_tools(&store, &capability_bindings, true, capabilities)
            .await
            .expect("available tools");
        source.replace(mcp_catalog(Some(CapabilityAvailabilityStatus::Unavailable)));
        let unavailable = build_model_tools(&store, &capability_bindings, true, capabilities)
            .await
            .expect("unavailable tools");

        assert!(!unavailable.tool_policy.allows_tool("mcp.mcp:docs.read"));
        assert!(
            unavailable
                .prompt_rows
                .iter()
                .all(|row| !row.contains("mcp.mcp:docs.read"))
        );
        assert!(
            unavailable
                .unavailable_rows
                .iter()
                .any(|row| row.contains("mcp:docs"))
        );
        if transport == ProviderToolTransport::Native {
            assert_eq!(available.provider_tools(), unavailable.provider_tools());
            let NoemaToolChoice::Allowed(allowed) =
                unavailable.allowed_tool_choice(NoemaAllowedToolsMode::Auto)
            else {
                panic!("expected provider-enforced allowed subset");
            };
            assert!(
                allowed
                    .tools
                    .iter()
                    .all(|tool| tool.as_str() != "mcp.mcp:docs.read")
            );
        } else {
            assert!(
                unavailable
                    .provider_tools()
                    .iter()
                    .all(|tool| { tool.name.as_str() != "mcp.mcp:docs.read" })
            );
        }
    }
}

#[tokio::test]
async fn background_roles_expose_read_tools_and_terminal_contracts_across_transports() {
    let store = crate::test_support::test_store().await;
    let (_, capability_bindings) = ready_mcp_source();

    for transport in [
        ProviderToolTransport::Native,
        ProviderToolTransport::NoemaEnvelope,
    ] {
        let capabilities = ProviderToolCapabilities {
            tool_transport: transport,
            native_tool_results: transport == ProviderToolTransport::Native,
            ..ProviderToolCapabilities::default()
        };
        for role in [ExecutionRole::TaskExecutor, ExecutionRole::TaskReviewer] {
            let tools =
                build_model_tools_for_role(&store, &capability_bindings, role, true, capabilities)
                    .await
                    .expect("role-aware tools");
            let provider_tools = tools.provider_tools();
            let names = provider_tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>();

            let terminal_tools = match role {
                ExecutionRole::TaskExecutor => {
                    vec![TASK_SUBMIT_RESULT_TOOL, TASK_REPORT_BLOCKED_TOOL]
                }
                ExecutionRole::TaskReviewer => vec![TASK_SUBMIT_REVIEW_TOOL],
                _ => unreachable!(),
            };
            assert_eq!(&names[..terminal_tools.len()], terminal_tools);
            for terminal in terminal_tools {
                assert!(tools.tool_policy.allows_tool(terminal));
            }
            assert!(tools.tool_policy.allows_tool("web.fetch"));
            assert!(tools.tool_policy.allows_tool(TASK_INSPECT_TOOL));
            assert!(!tools.tool_policy.allows_tool(TASK_RESUME_TOOL));
            if role == ExecutionRole::TaskExecutor {
                assert!(tools.tool_policy.allows_tool("artifact.create_local_file"));
                assert!(!tools.tool_policy.allows_tool(TASK_READ_ARTIFACT_TOOL));
            } else {
                assert!(!tools.tool_policy.allows_tool("artifact.create_local_file"));
                assert!(tools.tool_policy.allows_tool(TASK_READ_ARTIFACT_TOOL));
            }
            assert!(!tools.tool_policy.allows_tool("task.delegate"));
        }
    }
}
