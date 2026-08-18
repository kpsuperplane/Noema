use std::sync::{Arc, RwLock};

use super::*;
use noema_capabilities::{
    CapabilityBindingSource, CapabilityCatalogResult, CapabilityDestination,
    CapabilityExecutionDecision, CapabilityFuture, CapabilityScope, CapabilityServiceContext,
    CapabilityToolBehavior, OmitPayloadSanitizer, OperationToken, PayloadSanitizer,
};
use noema_providers::{
    ProviderCapabilityAccountReference, ProviderToolCapabilities, ProviderToolSchemaDialect,
    ProviderToolTransport,
};
use serde_json::json;

#[test]
fn runtime_binding_checks_its_published_input_rules() {
    let binding = runtime_binding(
        web_search_tool_spec().expect("search spec"),
        ToolAccessClass::ReadOnly,
        BindingPersistence::Redacted,
    )
    .expect("binding");
    assert!(binding.accepts_arguments(&json!({"query":"reliability"})));
    assert!(!binding.accepts_arguments(&json!({"query":7})));
    assert!(!binding.accepts_arguments(&json!({"query":"reliability","unknown":true})));
}

#[test]
fn native_memory_payload_persistence_omits_page_bodies_and_search_snippets() {
    let sanitizer = NativeMemoryPayloadSanitizer;
    assert_eq!(
        sanitizer.persist_output(&json!({
            "page": {
                "id": "memory:human:people.md",
                "path": "people.md",
                "hash": "abc",
                "body": "private page body"
            }
        })),
        Some(json!({
            "page_ref": {
                "id": "memory:human:people.md",
                "path": "people.md",
                "hash": "abc"
            }
        }))
    );
    assert_eq!(
        sanitizer.persist_output(&json!({
            "pages": [{
                "id": "memory:human:people.md",
                "path": "people.md",
                "hash": "abc",
                "snippet": "private search excerpt"
            }]
        })),
        Some(json!({
            "pages": [{
                "id": "memory:human:people.md",
                "path": "people.md",
                "hash": "abc"
            }]
        }))
    );
}

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
fn capability_scope_and_destination_map_to_role_access_separately() {
    let binding = |scope| {
        CapabilityBinding::new(
            ToolSpec::new("fixture.write", "Write.", json!({"type":"object"})).expect("spec"),
            CapabilityTarget::new(
                InvokerKey::new("fixture"),
                noema_capabilities::OperationToken::new("write"),
            ),
            CapabilityToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            scope,
            Arc::new(|_: &serde_json::Value| true),
            Arc::new(RedactingPayloadSanitizer),
        )
    };
    assert_eq!(
        capability_access_class(&binding(CapabilityScope::ExecutionOwned)),
        ToolAccessClass::TaskOwnedWrite
    );
    assert_eq!(
        capability_access_class(&binding(CapabilityScope::ConversationOwned)),
        ToolAccessClass::ConversationWrite
    );
    assert_eq!(
        capability_access_class(&binding(CapabilityScope::Global)),
        ToolAccessClass::Internal
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
            CapabilityToolBehavior {
                read_only: true,
                idempotent: true,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::Global,
            Arc::new(|_: &serde_json::Value| true),
            Arc::new(OmitPayloadSanitizer),
        ))
        .expect("unique MCP test binding");
    if availability == Some(CapabilityAvailabilityStatus::Disabled) {
        builder
            .add(
                CapabilityBinding::new(
                    ToolSpec::new(
                        "enable.mcp.mcp:docs.read",
                        "Ask the human to enable the document reader.",
                        json!({"type":"object","properties":{},"additionalProperties":false}),
                    )
                    .expect("enablement spec"),
                    CapabilityTarget::new(
                        InvokerKey::new("mcp"),
                        OperationToken::new("enable-test-mcp-authority"),
                    ),
                    CapabilityToolBehavior {
                        read_only: false,
                        idempotent: true,
                        destructive: false,
                        open_world: false,
                    },
                    CapabilityExecutionDecision::HumanReview,
                    CapabilityScope::Global,
                    Arc::new(|arguments: &serde_json::Value| {
                        arguments.as_object().is_some_and(serde_json::Map::is_empty)
                    }),
                    Arc::new(OmitPayloadSanitizer),
                )
                .with_destination(
                    CapabilityDestination::new("mcp", "mcp:docs", None::<String>, "generation:v1")
                        .expect("destination"),
                ),
            )
            .expect("unique enablement binding");
    }
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

fn connector_setup_source() -> CapabilityBindingSourceHandle {
    let mut builder = CapabilityCatalogBuilder::new();
    for (name, invoker, token, read_only) in [
        (
            "adapter.propose_definition",
            "adapter_json_v1",
            "adapter-setup-v1:propose-definition",
            false,
        ),
        ("fixture.global_write", "fixture", "write", false),
    ] {
        builder
            .add(CapabilityBinding::new(
                ToolSpec::new(name, "Connector setup fixture.", json!({"type": "object"}))
                    .expect("setup spec"),
                CapabilityTarget::new(InvokerKey::new(invoker), OperationToken::new(token)),
                CapabilityToolBehavior {
                    read_only,
                    idempotent: read_only,
                    destructive: false,
                    open_world: false,
                },
                CapabilityExecutionDecision::ExecuteImmediately,
                CapabilityScope::Global,
                Arc::new(|_: &serde_json::Value| true),
                Arc::new(RedactingPayloadSanitizer),
            ))
            .expect("unique setup binding");
    }
    let source = TestCapabilityBindingSource::default();
    source.replace(CapabilityCatalogResult {
        snapshot: builder.build(),
        availability_notices: Vec::new(),
    });
    source.handle()
}

#[test]
fn retained_catalog_never_grows_or_redirects_for_native_tools() {
    let initial = synthetic_model_tools(
        ProviderToolTransport::Native,
        [
            ("stable", "initial-target", true),
            ("initially-unavailable", "initial-unavailable-target", false),
        ],
    );
    let later = synthetic_model_tools(
        ProviderToolTransport::Native,
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
            .all(|row| !row.contains("newly-discovered") && !row.contains("initially-unavailable"))
    );
    assert_eq!(
        retained.prompt_rows,
        vec!["- builtin\tstable\tstable description"]
    );
}

#[test]
fn service_context_is_deduplicated_without_changing_tool_descriptions() {
    let mut builder = CapabilityCatalogBuilder::new();
    let destination = CapabilityDestination::new("mcp", "mcp:dex", None::<String>, "generation:v1")
        .expect("destination");
    let context = CapabilityServiceContext::new(
        "Dex",
        Some("Dex Personal CRM: Search contacts and correspondence."),
    )
    .expect("service context")
    .with_connection_label("person@example.test")
    .expect("account label");
    let mut policy = ToolPolicy::for_role(ExecutionRole::PrimaryConversation);
    let mut prompt_kinds = BTreeMap::new();
    for (name, description) in [
        ("dex.search_contacts", "Search contacts."),
        (
            "dex.search_emails",
            "Search connected account correspondence.",
        ),
    ] {
        let spec = ToolSpec::new(name, description, json!({"type": "object"})).expect("spec");
        builder
            .add(
                CapabilityBinding::new(
                    spec,
                    CapabilityTarget::new(
                        InvokerKey::new("mcp"),
                        noema_capabilities::OperationToken::new(name),
                    ),
                    CapabilityToolBehavior {
                        read_only: true,
                        idempotent: true,
                        destructive: false,
                        open_world: false,
                    },
                    CapabilityExecutionDecision::ExecuteImmediately,
                    CapabilityScope::Global,
                    Arc::new(|_: &serde_json::Value| true),
                    Arc::new(RedactingPayloadSanitizer),
                )
                .with_destination(destination.clone())
                .with_service_context(context.clone()),
            )
            .expect("binding");
        policy.allow_tool_name(name);
        prompt_kinds.insert(name.to_string(), ModelToolPromptKind::Capability);
    }
    let bindings = builder.build();
    let provider_tools = bindings
        .provider_specs()
        .into_iter()
        .map(Into::into)
        .collect::<Vec<_>>();
    let rows = catalog_prompt_rows(
        &provider_tools,
        &bindings,
        &prompt_kinds,
        &policy,
        ProviderToolTransport::Native,
    );

    let service_rows = rows
        .iter()
        .filter(|row| row.starts_with("- service\t"))
        .count();
    assert_eq!(service_rows, 1);
    assert!(
        rows[0].contains("name=\"Dex\"")
            && rows[0].contains("connection_label=\"person@example.test\"")
            && rows[0].contains("Dex Personal CRM")
    );
    let owned_tools = rows
        .iter()
        .filter(|row| row.starts_with("- capability\t") && row.contains("service=mcp:dex"))
        .count();
    assert_eq!(owned_tools, 2);
    assert!(
        provider_tools[0].description == "Search contacts."
            && provider_tools[1].description == "Search connected account correspondence."
    );
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
                CapabilityToolBehavior {
                    read_only: true,
                    idempotent: true,
                    destructive: false,
                    open_world: false,
                },
                CapabilityExecutionDecision::ExecuteImmediately,
                CapabilityScope::Global,
                Arc::new(|_: &serde_json::Value| true),
                Arc::new(RedactingPayloadSanitizer),
            ))
            .expect("unique binding");
        if allowed {
            policy.allow_tool_name(name);
        }
        prompt_kinds.insert(name.to_string(), ModelToolPromptKind::Builtin);
    }
    let bindings = builder.build();
    let provider_tools = bindings
        .provider_specs()
        .into_iter()
        .map(Into::into)
        .collect::<Vec<_>>();
    let prompt_rows = catalog_prompt_rows(
        &provider_tools,
        &bindings,
        &prompt_kinds,
        &policy,
        transport,
    );
    ModelTools {
        transport,
        bindings,
        provider_tools,
        hosted_web_search: false,
        prompt_rows,
        unavailable_rows: Vec::new(),
        prompt_kinds,
        tool_policy: policy,
    }
}

#[tokio::test]
async fn complete_catalog_is_stable_for_native_transport() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let (_, capability_bindings) = ready_mcp_source();

    for transport in [ProviderToolTransport::Native] {
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
                "read_memory_page",
                "search_memory",
                "file.parse",
                "update_own_name",
                "artifact.create_local_file",
                "file.download",
                "noema.present_multiple_choice",
                "noema.present_a2ui",
                "task.capture",
                "task.list",
                "task.update",
                "task.queue",
                "task.schedule",
                "task.reschedule",
                "task.unschedule",
                "task.schedule.run_now",
                "task.recurrence.update",
                "task.recurrence.pause",
                "task.recurrence.resume",
                "task.recurrence.skip_next",
                "task.recurrence.end",
                "task.recurrence.run_now",
                "task.delegate",
                "task.answer",
                "task.retry",
                "task.cancel",
                "task.reopen",
                "project.create",
                "project.list",
                "project.update",
                "project.archive",
                "project.reopen",
                "web.search",
                "web.fetch",
                "web.browse.open",
                "web.browse.snapshot",
                "web.browse.interact",
                "web.browse.wait",
                "web.browse.history",
                "web.browse.close",
                "mcp.mcp:docs.read",
            ]
        );
        assert!(tools.provider_tools().iter().any(|tool| {
            tool.name.as_str() == "mcp.mcp:docs.read" && tool.description == "Read a document."
        }));
        let provider_tools = tools.provider_tools();
        let scheduling = provider_tools
            .iter()
            .find(|tool| tool.name.as_str() == "task.schedule")
            .unwrap();
        assert_eq!(
            scheduling.input_schema.as_value()["required"],
            json!([
                "task_id",
                "expected_revision",
                "expected_generation",
                "scheduled_for"
            ])
        );
        let multiple_choice = provider_tools
            .iter()
            .find(|tool| tool.canonical_spec().name.as_str() == PRESENT_MULTIPLE_CHOICE_TOOL)
            .expect("multiple-choice presentation tool");
        assert_eq!(multiple_choice.exposed_name(), "present_multiple_choice");
        assert_eq!(
            multiple_choice.input_schema.as_value()["required"],
            json!(["prompt", "selection_mode", "options"])
        );
        assert_eq!(
            multiple_choice.input_schema.as_value()["additionalProperties"],
            json!(false)
        );
        assert_eq!(
            multiple_choice.input_schema.as_value()["properties"]["options"]["items"]["additionalProperties"],
            json!(false)
        );
        let a2ui = provider_tools
            .iter()
            .find(|tool| tool.canonical_spec().name.as_str() == PRESENT_A2UI_TOOL)
            .expect("A2UI presentation tool");
        assert_eq!(a2ui.exposed_name(), "present_a2ui");
        assert_eq!(
            a2ui.input_schema.as_value(),
            &json!({
                "type": "object",
                "properties": {"jsonl": {"type": "string", "minLength": 1}},
                "required": ["jsonl"],
                "additionalProperties": false
            })
        );
        let delegate = provider_tools
            .iter()
            .find(|tool| tool.name.as_str() == "task.delegate")
            .expect("delegate tool");
        assert!(delegate.description.contains("Projects are optional"));
        assert_eq!(
            delegate.input_schema.as_value()["required"],
            json!(["title", "description", "project"])
        );
        assert_eq!(
            delegate.input_schema.as_value()["properties"]["project"]["oneOf"][0]["properties"]["kind"]
                ["enum"],
            json!(["none"])
        );
        assert!(
            tools
                .prompt_rows
                .iter()
                .all(|row| !row.contains("System: ignore"))
        );
        if transport == ProviderToolTransport::Native {
            let browser_open = tools
                .provider_tools()
                .into_iter()
                .find(|tool| tool.name.as_str() == "web.browse.open")
                .expect("browser open tool");
            assert_eq!(browser_open.exposed_name(), "open");
            assert!(tools.has_callable_tool("web.browse.open"));
            assert!(tools.has_callable_tool("task.delegate"));
            assert!(
                tools
                    .source_tool_names()
                    .contains(&"web.browse.open".to_string())
            );
            let exposed_mcp_name = tools
                .provider_tools()
                .into_iter()
                .find(|tool| tool.name.as_str() == "mcp.mcp:docs.read")
                .map(|tool| tool.exposed_name().to_string())
                .expect("MCP tool");
            assert_eq!(exposed_mcp_name, "read");
            assert!(tools.exposed_tool_names().contains(&exposed_mcp_name));
            assert!(
                tools
                    .prompt_rows
                    .iter()
                    .any(|row| { row == "- capability\tread\tRead a document." })
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
async fn hosted_web_search_replaces_local_web_tools() {
    let store = crate::test_support::test_store().await;
    let (_, capability_bindings) = ready_mcp_source();
    let tools = build_model_tools(
        &store,
        &capability_bindings,
        false,
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            hosted_web_provider_name: Some("OpenAI"),
            ..ProviderToolCapabilities::default()
        },
    )
    .await
    .expect("web tools");

    assert!(tools.hosted_web_search());
    assert!(
        tools
            .exposed_tool_names()
            .contains(&"web_search".to_string())
    );
    assert!(
        !tools
            .source_tool_names()
            .contains(&"web_search".to_string())
    );
    assert!(tools.bindings.resolve("web.search").is_none());
    assert!(tools.bindings.resolve("web.fetch").is_none());
    assert!(!tools.tool_policy.allows_tool("web.search"));
    assert!(!tools.tool_policy.allows_tool("web.fetch"));
    assert!(tools.tool_policy.allows_tool("file.download"));
    assert!(tools.tool_policy.allows_tool("file.parse"));
    assert!(
        tools
            .bindings
            .resolve("file.download")
            .unwrap()
            .destination()
            .is_some()
    );
}

#[tokio::test]
async fn explicit_web_provider_selection_replaces_hosted_web_tools() {
    let store = crate::test_support::test_store().await;
    crate::test_support::save_provider_capability_assignment_for_tests(
        &store,
        "web.search",
        "web.search",
        ProviderCapabilityAccountReference::persisted("provider_account:duckduckgo_public:system"),
    )
    .await;
    let (_, capability_bindings) = ready_mcp_source();

    let tools = build_model_tools(
        &store,
        &capability_bindings,
        false,
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            hosted_web_provider_name: Some("OpenAI"),
            ..ProviderToolCapabilities::default()
        },
    )
    .await
    .expect("configured web tools");

    assert!(!tools.hosted_web_search());
    assert!(tools.bindings.resolve("web.search").is_some());
    assert!(tools.bindings.resolve("web.fetch").is_some());
    assert!(tools.tool_policy.allows_tool("web.search"));
    assert!(tools.tool_policy.allows_tool("web.fetch"));
    assert!(tools.tool_policy.allows_tool("file.download"));
}

#[tokio::test]
async fn planner_catalog_contains_task_file_tools() {
    let store = crate::test_support::test_store().await;
    let (_, capability_bindings) = ready_mcp_source();
    let tools = build_model_tools_for_role(
        &store,
        &capability_bindings,
        ExecutionRole::TaskPlanner,
        true,
        ProviderToolCapabilities {
            tool_transport: ProviderToolTransport::Native,
            native_tool_results: true,
            hosted_web_provider_name: Some("OpenAI"),
            ..ProviderToolCapabilities::default()
        },
    )
    .await
    .expect("planner tools");
    assert_eq!(
        tools
            .provider_tools()
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "file.parse",
            "task.finish_planning",
            "task.report_blocked",
            "task.files.list",
            "task.files.read",
            "task.files.write",
            "task.files.delete",
        ]
    );
    assert!(tools.tool_policy.allows_tool(TASK_FINISH_PLANNING_TOOL));
    assert!(tools.tool_policy.allows_tool(TASK_REPORT_BLOCKED_TOOL));
    assert!(tools.tool_policy.allows_tool("file.parse"));
    assert!(!tools.tool_policy.allows_tool(TASK_LIST_TOOL));
    assert!(!tools.tool_policy.allows_tool("search_memory"));
    assert!(!tools.tool_policy.allows_tool("web.fetch"));
    assert!(!tools.tool_policy.allows_tool("file.download"));
    assert!(!tools.tool_policy.allows_tool("mcp.mcp:docs.read"));
    assert!(!tools.tool_policy.allows_tool("artifact.create_local_file"));
    assert!(!tools.hosted_web_search());
}

#[tokio::test]
async fn transient_outages_preserve_native_catalog_during_outages() {
    let store = crate::test_support::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let (source, capability_bindings) = ready_mcp_source();
    for transport in [ProviderToolTransport::Native] {
        source.replace(mcp_catalog(None));
        let capabilities = ProviderToolCapabilities {
            tool_transport: transport,
            allowed_tools: true,
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
        source.replace(mcp_catalog(Some(CapabilityAvailabilityStatus::Disabled)));
        let disabled = build_model_tools(&store, &capability_bindings, true, capabilities)
            .await
            .expect("disabled tools");
        assert!(
            disabled.unavailable_rows.iter().any(|row| {
                row.ends_with("status=disabled\tenable_with=enable.mcp.mcp:docs.read")
            })
        );
        assert!(!disabled.tool_policy.allows_tool("mcp.mcp:docs.read"));
        assert!(disabled.tool_policy.allows_tool("enable.mcp.mcp:docs.read"));
    }
}

#[tokio::test]
async fn background_roles_expose_read_tools_and_terminals_for_native_tools() {
    let store = crate::test_support::test_store().await;
    let (_, capability_bindings) = ready_mcp_source();

    for transport in [ProviderToolTransport::Native] {
        let capabilities = ProviderToolCapabilities {
            tool_transport: transport,
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
                    vec![TASK_FINISH_EXECUTION_TOOL, TASK_CONTINUE_EXECUTION_TOOL]
                }
                ExecutionRole::TaskReviewer => vec![TASK_FINISH_REVIEW_TOOL],
                _ => unreachable!(),
            };
            assert_eq!(&names[..terminal_tools.len()], terminal_tools);
            for terminal in terminal_tools {
                assert!(tools.tool_policy.allows_tool(terminal));
            }
            assert_eq!(
                tools.tool_policy.allows_tool("web.fetch"),
                role == ExecutionRole::TaskExecutor
            );
            assert_eq!(
                tools.tool_policy.allows_tool("file.download"),
                role == ExecutionRole::TaskExecutor
            );
            assert_eq!(
                tools.tool_policy.allows_tool(TASK_LIST_TOOL),
                role == ExecutionRole::TaskExecutor
            );
            assert!(!tools.tool_policy.allows_tool(TASK_ANSWER_TOOL));
            assert!(!tools.tool_policy.allows_tool(PRESENT_MULTIPLE_CHOICE_TOOL));
            assert!(!tools.tool_policy.allows_tool(PRESENT_A2UI_TOOL));
            if role == ExecutionRole::TaskExecutor {
                assert!(tools.tool_policy.allows_tool("artifact.create_local_file"));
            } else {
                assert!(!tools.tool_policy.allows_tool("artifact.create_local_file"));
            }
            assert!(tools.tool_policy.allows_tool(TASK_READ_ARTIFACT_TOOL));
            assert!(!tools.tool_policy.allows_tool("task.delegate"));
        }
    }
}

#[tokio::test]
async fn task_executor_can_submit_only_the_exact_pending_connector_proposal() {
    let store = crate::test_support::test_store().await;
    let source = connector_setup_source();
    let capabilities = ProviderToolCapabilities {
        tool_transport: ProviderToolTransport::Native,
        native_tool_results: true,
        ..ProviderToolCapabilities::default()
    };

    let executor = build_model_tools_for_role(
        &store,
        &source,
        ExecutionRole::TaskExecutor,
        false,
        capabilities,
    )
    .await
    .expect("executor tools");
    assert!(
        executor
            .tool_policy
            .allows_tool("adapter.propose_definition")
    );
    assert!(!executor.tool_policy.allows_tool("fixture.global_write"));
    assert!(!is_background_connector_proposal(
        ExecutionRole::TaskReviewer,
        executor
            .bindings
            .resolve("adapter.propose_definition")
            .expect("proposal binding"),
    ));
}
