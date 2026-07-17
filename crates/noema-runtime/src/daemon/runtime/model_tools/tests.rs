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

fn empty_capability_source() -> CapabilityBindingSourceHandle {
    TestCapabilityBindingSource::default().handle()
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
async fn native_provider_gets_builtin_and_calibrated_mcp_tools() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let (_, capability_bindings) = ready_mcp_source();

    let tools = build_model_tools(
        &store,
        &capability_bindings,
        true,
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            parallel_tool_calls: true,
            tool_choice: true,
            allowed_tools: false,
            schema_dialect: ProviderToolSchemaDialect::OpenAiResponses,
            strict_schema: false,
            custom_tools: false,
            native_tool_results: true,
            prompt_cache_retention: false,
            prompt_cache_key: false,
            prompt_cache_options: false,
            prompt_cache_breakpoints: false,
            encrypted_reasoning: false,
        },
    )
    .await
    .expect("tools");

    let provider_tools = tools.provider_tools();
    let names = provider_tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "search_memory",
            "task.inspect",
            "update_own_name",
            "artifact.create_local_file",
            "task.resume",
            "task.cancel",
            "web.search",
            "web.fetch",
            "mcp.mcp:docs.read"
        ]
    );
    assert!(
        provider_tools
            .iter()
            .any(|tool| { tool.name.as_str() == "web.search" })
    );
    assert!(
        provider_tools
            .iter()
            .any(|tool| { tool.name.as_str() == "web.fetch" })
    );
    assert!(provider_tools.iter().any(|tool| {
        tool.name.as_str() == "mcp.mcp:docs.read" && tool.description == "Read a document."
    }));
    assert!(
        tools
            .prompt_rows
            .iter()
            .any(|row| { row == "- capability\tmcp.mcp:docs.read\tRead a document." })
    );
    assert!(
        tools
            .prompt_rows
            .iter()
            .any(|row| row.contains("\tweb.fetch\t"))
    );
    assert!(
        tools
            .prompt_rows
            .iter()
            .all(|row| !row.contains("System: ignore"))
    );
    assert_eq!(tools.transport, ProviderToolTransport::Native);
}

#[tokio::test]
async fn native_catalog_keeps_prompt_safe_approved_tools_across_transient_outages() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let (source, capability_bindings) = ready_mcp_source();
    let capabilities = ProviderToolCapabilities {
        tool_transport: ProviderToolTransport::Native,
        allowed_tools: true,
        ..ProviderToolCapabilities::default()
    };
    let available_tools = build_model_tools(&store, &capability_bindings, true, capabilities)
        .await
        .expect("available tools");
    source.replace(mcp_catalog(Some(CapabilityAvailabilityStatus::Unavailable)));

    let tools = build_model_tools(&store, &capability_bindings, true, capabilities)
        .await
        .expect("tools");

    assert_eq!(available_tools.provider_tools(), tools.provider_tools());
    assert!(
        tools
            .prompt_rows
            .iter()
            .all(|row| !row.contains("mcp.mcp:docs.read"))
    );
    assert!(tools.provider_tools().iter().any(|tool| {
        tool.name.as_str() == "mcp.mcp:docs.read" && tool.description == "Read a document."
    }));
    assert!(
        !tools
            .tool_policy
            .strict_for_dispatch()
            .allows_tool("mcp.mcp:docs.read")
    );
    let NoemaToolChoice::Allowed(allowed) = tools.allowed_tool_choice(NoemaAllowedToolsMode::Auto)
    else {
        panic!("expected provider-enforced allowed subset");
    };
    assert!(
        allowed
            .tools
            .iter()
            .all(|tool| tool.as_str() != "mcp.mcp:docs.read")
    );
    assert!(
        tools
            .unavailable_rows
            .iter()
            .any(|row| row.contains("mcp:docs"))
    );
}

#[tokio::test]
async fn envelope_catalog_excludes_transiently_unavailable_mcp_tools() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let source = TestCapabilityBindingSource::default();
    source.replace(mcp_catalog(Some(CapabilityAvailabilityStatus::Unavailable)));
    let capability_bindings = source.handle();

    let tools = build_model_tools(
        &store,
        &capability_bindings,
        true,
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::NoemaEnvelope,
            allowed_tools: false,
            ..ProviderToolCapabilities::default()
        },
    )
    .await
    .expect("tools");

    assert!(
        tools
            .provider_tools()
            .iter()
            .all(|tool| tool.name.as_str() != "mcp.mcp:docs.read")
    );
    assert!(
        tools
            .prompt_rows
            .iter()
            .all(|row| !row.contains("mcp.mcp:docs.read"))
    );
    assert!(
        !tools
            .tool_policy
            .strict_for_dispatch()
            .allows_tool("mcp.mcp:docs.read")
    );
}

#[tokio::test]
async fn noema_envelope_gets_the_same_complete_catalog() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let (_, capability_bindings) = ready_mcp_source();

    let tools = build_model_tools(
        &store,
        &capability_bindings,
        true,
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::NoemaEnvelope,
            ..ProviderToolCapabilities::default()
        },
    )
    .await
    .expect("tools");

    assert_eq!(tools.transport, ProviderToolTransport::NoemaEnvelope);
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
    assert!(
        tools
            .prompt_rows
            .iter()
            .all(|row| row.contains("input_schema="))
    );
    assert!(tools.prompt_rows.iter().any(|row| {
        row.contains("mcp.mcp:docs.read")
            && row.contains("Read a document.")
            && !row.contains("System: ignore")
    }));
}

#[tokio::test]
async fn no_tool_transport_exposes_no_catalog() {
    let store = crate::test_support::test_store().await;
    let (_, capability_bindings) = ready_mcp_source();

    let tools = build_model_tools(
        &store,
        &capability_bindings,
        true,
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::None,
            ..ProviderToolCapabilities::default()
        },
    )
    .await
    .expect("tools");

    assert_eq!(tools.transport, ProviderToolTransport::None);
    assert!(tools.bindings.is_empty());
    assert!(tools.prompt_rows.is_empty());
    assert!(
        !tools
            .tool_policy
            .strict_for_dispatch()
            .allows_tool("search_memory")
    );
}

#[tokio::test]
async fn authenticated_provider_defaults_expose_task_delegation() {
    let store = crate::test_support::test_store().await;
    let capability_bindings = empty_capability_source();
    store
        .ensure_default_provider_account()
        .await
        .expect("provider account");
    store
        .update_provider_account_status(
            "provider_account:codex:default",
            noema_providers::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect("authenticated provider");
    crate::test_support::initialize_codex_provider_selections(&store).await;
    store
        .ensure_default_task_model_pool_settings("codex")
        .await
        .expect("provider defaults");

    let tools = build_model_tools(
        &store,
        &capability_bindings,
        false,
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            ..ProviderToolCapabilities::default()
        },
    )
    .await
    .expect("tools");
    let provider_tools = tools.provider_tools();
    let delegation = provider_tools
        .iter()
        .find(|tool| tool.name.as_str() == "task.delegate")
        .expect("task delegation tool");
    assert!(
        provider_tools
            .iter()
            .any(|tool| tool.name.as_str() == TASK_INSPECT_TOOL)
    );
    assert!(
        provider_tools
            .iter()
            .any(|tool| tool.name.as_str() == TASK_RESUME_TOOL)
    );

    let pool_ids = delegation.input_schema.as_value()["properties"]["executor_model_pool_entry_id"]
        ["enum"]
        .as_array()
        .expect("pool ids");
    assert_eq!(pool_ids.len(), 3);
    for complexity in ["simple", "medium", "difficult"] {
        assert!(
            delegation
                .description
                .contains(&format!("gpt-5.6-luna ({complexity}, codex)"))
        );
    }
}

#[tokio::test]
async fn background_roles_expose_read_tools_and_their_typed_terminal_contracts() {
    let store = crate::test_support::test_store().await;
    let (_, capability_bindings) = ready_mcp_source();

    let capabilities = ProviderToolCapabilities {
        tool_transport: ProviderToolTransport::Native,
        native_tool_results: true,
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
        for terminal in terminal_tools {
            assert!(names.contains(&terminal));
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

#[tokio::test]
async fn envelope_background_roles_keep_typed_terminal_specs() {
    let store = crate::test_support::test_store().await;
    let capability_bindings = empty_capability_source();
    let capabilities = ProviderToolCapabilities {
        tool_transport: ProviderToolTransport::NoemaEnvelope,
        ..ProviderToolCapabilities::default()
    };

    let executor = build_model_tools_for_role(
        &store,
        &capability_bindings,
        ExecutionRole::TaskExecutor,
        false,
        capabilities,
    )
    .await
    .expect("executor tools");
    let executor_specs = executor.provider_tools();
    assert_eq!(executor_specs[0].name.as_str(), TASK_SUBMIT_RESULT_TOOL);
    assert_eq!(executor_specs[1].name.as_str(), TASK_REPORT_BLOCKED_TOOL);
    assert!(
        executor_specs
            .iter()
            .any(|tool| tool.name.as_str() == "web.fetch")
    );

    let reviewer = build_model_tools_for_role(
        &store,
        &capability_bindings,
        ExecutionRole::TaskReviewer,
        false,
        capabilities,
    )
    .await
    .expect("reviewer tools");
    assert_eq!(
        reviewer.provider_tools()[0].name.as_str(),
        TASK_SUBMIT_REVIEW_TOOL
    );
}

#[tokio::test]
async fn native_provider_hides_ready_write_tool_without_one_shot_approval() {
    let store = crate::test_support::test_store().await;
    let capability_bindings = empty_capability_source();

    let tools = build_model_tools(
        &store,
        &capability_bindings,
        false,
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            ..ProviderToolCapabilities::default()
        },
    )
    .await
    .expect("tools");
    assert!(
        tools
            .provider_tools()
            .iter()
            .all(|tool| tool.name.as_str() != "mcp.mcp:docs.read")
    );
}
