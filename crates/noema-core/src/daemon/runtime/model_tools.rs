use noema_store::NoemaStore;

use crate::{
    agent_execution::{ExecutionRole, ToolAccessClass, ToolPolicy},
    daemon::{
        agent_name_tool::update_own_name_tool_spec,
        artifact_tool::artifact_create_local_file_tool_spec,
        task_artifact_tool::{TASK_READ_ARTIFACT_TOOL, task_read_artifact_tool_spec},
        task_tool::{
            TASK_CANCEL_TOOL, TASK_INSPECT_TOOL, TASK_REPORT_BLOCKED_TOOL, TASK_RESUME_TOOL,
            TASK_SUBMIT_RESULT_TOOL, TASK_SUBMIT_REVIEW_TOOL, task_cancel_tool_spec,
            task_delegate_tool_spec, task_inspect_tool_spec, task_report_blocked_tool_spec,
            task_resume_tool_spec, task_submit_result_tool_spec, task_submit_review_tool_spec,
        },
    },
    search::tool::web_search_tool_spec,
    web_fetch::tool::web_fetch_tool_spec,
};
use noema_capabilities::{
    ArtifactPayloadSanitizer, CapabilityAccess, CapabilityAvailabilityNotice,
    CapabilityAvailabilityStatus, CapabilityBinding, CapabilityBindingSourceError,
    CapabilityBindingSourceHandle, CapabilityCatalogBuilder, CapabilityCatalogSnapshot,
    CapabilityEffect, CapabilityScope, CapabilityTarget, InvokerKey, RedactingPayloadSanitizer,
    ToolContractError, ToolName, ToolSpec, WebFetchPayloadSanitizer,
};
use noema_memory::search_memory_tool_spec;
use noema_providers::{
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderToolCapabilities,
    ProviderToolTransport,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Copy)]
pub(in crate::daemon) enum ModelToolPromptKind {
    Builtin,
    Web,
    Capability,
}

#[derive(Debug, Clone)]
pub(in crate::daemon) struct ModelTools {
    /// Provider representation used for this catalog.
    pub(in crate::daemon) transport: ProviderToolTransport,
    /// Exact immutable role-filtered binding snapshot retained through every
    /// continuation that reuses the provider-visible tools.
    pub(in crate::daemon) bindings: CapabilityCatalogSnapshot,
    pub(in crate::daemon) prompt_rows: Vec<String>,
    pub(in crate::daemon) unavailable_rows: Vec<String>,
    pub(in crate::daemon) prompt_kinds: BTreeMap<String, ModelToolPromptKind>,
    /// Exact names advertised for this role and safe to dispatch.
    pub(in crate::daemon) tool_policy: ToolPolicy,
}

pub(super) async fn build_model_tools(
    store: &NoemaStore,
    capability_bindings: &CapabilityBindingSourceHandle,
    include_agent_name_tool: bool,
    capabilities: ProviderToolCapabilities,
) -> Result<ModelTools, ToolContractError> {
    build_model_tools_for_role(
        store,
        capability_bindings,
        ExecutionRole::PrimaryConversation,
        include_agent_name_tool,
        capabilities,
    )
    .await
}

/// Build tools for one explicit execution role.
///
/// The primary wrapper above preserves the current foreground call sites.
/// Background execution should call this role-aware entry point and pass the
/// resulting policy to dispatch as well as to the provider request builder.
pub(super) async fn build_model_tools_for_role(
    store: &NoemaStore,
    capability_bindings: &CapabilityBindingSourceHandle,
    role: ExecutionRole,
    include_agent_name_tool: bool,
    capabilities: ProviderToolCapabilities,
) -> Result<ModelTools, ToolContractError> {
    let transport = capabilities.tool_transport;
    let capability_catalog = capability_bindings
        .catalog()
        .await
        .map_err(binding_source_tool_error)?;
    let unavailable_rows = capability_catalog
        .availability_notices
        .iter()
        .filter(|notice| notice.status != CapabilityAvailabilityStatus::Disabled)
        .map(render_availability_notice)
        .collect();
    if transport == ProviderToolTransport::None {
        return Ok(ModelTools {
            transport,
            bindings: CapabilityCatalogSnapshot::default(),
            prompt_rows: Vec::new(),
            unavailable_rows,
            prompt_kinds: BTreeMap::new(),
            tool_policy: ToolPolicy::for_role(role),
        });
    }

    let mut builtin_tools = match role {
        ExecutionRole::TaskExecutor => {
            vec![
                task_submit_result_tool_spec()?,
                task_report_blocked_tool_spec()?,
            ]
        }
        ExecutionRole::TaskReviewer => vec![task_submit_review_tool_spec()?],
        ExecutionRole::PrimaryConversation => Vec::new(),
    };
    builtin_tools.extend(builtin_tool_specs(include_agent_name_tool)?);
    if role == ExecutionRole::PrimaryConversation {
        builtin_tools.push(task_resume_tool_spec()?);
        builtin_tools.push(task_cancel_tool_spec()?);
        let pool_entries = store
            .list_usable_task_model_pool_entries()
            .await
            .map_err(store_tool_error)?;
        if !pool_entries.is_empty() {
            builtin_tools.push(task_delegate_tool_spec(&pool_entries)?);
        }
    } else if role == ExecutionRole::TaskReviewer {
        builtin_tools.push(task_read_artifact_tool_spec()?);
    }
    let web_search_tool = web_search_tool_spec()?;
    let web_fetch_tool = web_fetch_tool_spec()?;
    let mut tool_policy = ToolPolicy::for_role(role);
    let mut declared_builtin_tools = Vec::new();
    for tool in builtin_tools {
        let class = builtin_tool_access_class(role, tool.name.as_str());
        if tool_policy.declare_tool(tool.name.as_str(), class) {
            declared_builtin_tools.push((tool, class));
        }
    }
    let declared_web_tools = [web_search_tool, web_fetch_tool]
        .into_iter()
        .filter(|tool| tool_policy.declare_tool(tool.name.as_str(), ToolAccessClass::ReadOnly))
        .collect::<Vec<_>>();

    let mut catalog = CapabilityCatalogBuilder::new();
    let mut prompt_kinds = BTreeMap::new();
    for (tool, class) in declared_builtin_tools {
        prompt_kinds.insert(tool.name.as_str().to_string(), ModelToolPromptKind::Builtin);
        let persistence = if tool.name.as_str() == "artifact.create_local_file" {
            BindingPersistence::Artifact
        } else {
            BindingPersistence::Redacted
        };
        add_binding(&mut catalog, runtime_binding(tool, class, persistence))?;
    }
    for tool in declared_web_tools {
        prompt_kinds.insert(tool.name.as_str().to_string(), ModelToolPromptKind::Web);
        let persistence = if tool.name.as_str() == noema_capabilities::web::fetch::WEB_FETCH_TOOL {
            BindingPersistence::WebFetch
        } else {
            BindingPersistence::Redacted
        };
        add_binding(
            &mut catalog,
            runtime_binding(tool, ToolAccessClass::ReadOnly, persistence),
        )?;
    }
    let unavailable_capabilities = capability_catalog
        .availability_notices
        .iter()
        .filter_map(|notice| notice.capability.as_ref().map(ToolName::as_str))
        .collect::<std::collections::HashSet<_>>();
    for binding in capability_catalog.snapshot.iter() {
        let callable = !unavailable_capabilities.contains(binding.spec().name.as_str());
        let Some(access_class) = capability_access_class(binding.access()) else {
            continue;
        };
        if !tool_policy.allows_class(access_class) {
            continue;
        }
        if !callable && !capabilities.allowed_tools {
            continue;
        }
        add_binding(&mut catalog, binding.clone())?;
        prompt_kinds.insert(
            binding.spec().name.as_str().to_string(),
            ModelToolPromptKind::Capability,
        );
        if !callable {
            continue;
        }
        let spec = binding.spec();
        tool_policy.declare_tool(spec.name.as_str(), access_class);
    }

    let bindings = catalog.build();
    let prompt_rows = catalog_prompt_rows(&bindings, &prompt_kinds, &tool_policy, transport);

    Ok(ModelTools {
        transport,
        bindings,
        prompt_rows,
        unavailable_rows,
        prompt_kinds,
        tool_policy,
    })
}

impl ModelTools {
    pub(in crate::daemon) fn empty(role: ExecutionRole) -> Self {
        Self {
            transport: ProviderToolTransport::None,
            bindings: CapabilityCatalogSnapshot::default(),
            prompt_rows: Vec::new(),
            unavailable_rows: Vec::new(),
            prompt_kinds: BTreeMap::new(),
            tool_policy: ToolPolicy::for_role(role).strict_for_dispatch(),
        }
    }

    pub(in crate::daemon) fn has_callable_tools(&self) -> bool {
        let strict_policy = self.tool_policy.strict_for_dispatch();
        self.bindings
            .iter()
            .any(|binding| strict_policy.allows_tool(binding.spec().name.as_str()))
    }

    /// Retain the exact initially advertised bindings while applying the
    /// continuation's narrower policy. Provider request catalogs may stay
    /// stable for allowed-tools APIs, but execution authority can only shrink.
    pub(in crate::daemon) fn retained_catalog_with_policy(
        initial: &Self,
        continuation: &Self,
    ) -> Self {
        let tool_policy = initial
            .tool_policy
            .intersect_allowed_names(&continuation.tool_policy)
            .strict_for_dispatch();
        Self {
            transport: initial.transport,
            bindings: initial.bindings.clone(),
            prompt_rows: catalog_prompt_rows(
                &initial.bindings,
                &initial.prompt_kinds,
                &tool_policy,
                initial.transport,
            ),
            unavailable_rows: initial.unavailable_rows.clone(),
            prompt_kinds: initial.prompt_kinds.clone(),
            tool_policy,
        }
    }

    pub(in crate::daemon) fn callable_tool_names(&self) -> Vec<noema_capabilities::ToolName> {
        let strict_policy = self.tool_policy.strict_for_dispatch();
        self.bindings
            .iter()
            .filter(|binding| strict_policy.allows_tool(binding.spec().name.as_str()))
            .map(|binding| binding.spec().name.clone())
            .collect()
    }

    pub(in crate::daemon) fn provider_tools(&self) -> Vec<ToolSpec> {
        if self.transport != ProviderToolTransport::None {
            self.bindings.provider_specs()
        } else {
            Vec::new()
        }
    }

    pub(in crate::daemon) fn policy_filtered_provider_tools(&self) -> Vec<ToolSpec> {
        if self.transport == ProviderToolTransport::None {
            return Vec::new();
        }
        let strict_policy = self.tool_policy.strict_for_dispatch();
        self.bindings
            .iter()
            .filter(|binding| strict_policy.allows_tool(binding.spec().name.as_str()))
            .map(|binding| binding.spec().clone())
            .collect()
    }

    pub(in crate::daemon) fn allowed_tool_choice(
        &self,
        mode: NoemaAllowedToolsMode,
    ) -> NoemaToolChoice {
        let tools = self.callable_tool_names();
        if tools.is_empty() {
            NoemaToolChoice::None
        } else {
            NoemaToolChoice::Allowed(NoemaAllowedTools { mode, tools })
        }
    }
}

fn builtin_tool_access_class(role: ExecutionRole, name: &str) -> ToolAccessClass {
    match name {
        // This tool is read-only and can be safely used by executor/reviewer
        // roles once their scope context is supplied by the task runtime.
        "search_memory" | TASK_INSPECT_TOOL | TASK_READ_ARTIFACT_TOOL => ToolAccessClass::ReadOnly,
        "artifact.create_local_file" if role == ExecutionRole::TaskExecutor => {
            ToolAccessClass::TaskOwnedWrite
        }
        "artifact.create_local_file" => ToolAccessClass::ConversationWrite,
        // Renaming the primary identity is a foreground-only control action.
        "update_own_name" | TASK_RESUME_TOOL | TASK_CANCEL_TOOL => ToolAccessClass::Internal,
        TASK_SUBMIT_RESULT_TOOL | TASK_REPORT_BLOCKED_TOOL => ToolAccessClass::ExecutorTerminal,
        TASK_SUBMIT_REVIEW_TOOL => ToolAccessClass::ReviewerTerminal,
        _ => ToolAccessClass::Internal,
    }
}

fn capability_access_class(access: CapabilityAccess) -> Option<ToolAccessClass> {
    match (access.effect, access.scope) {
        (CapabilityEffect::ReadOnly, _) => Some(ToolAccessClass::ReadOnly),
        (CapabilityEffect::Mutating, CapabilityScope::ExecutionOwned) => {
            Some(ToolAccessClass::TaskOwnedWrite)
        }
        (CapabilityEffect::Mutating, CapabilityScope::ConversationOwned) => {
            Some(ToolAccessClass::ConversationWrite)
        }
        (CapabilityEffect::Mutating, CapabilityScope::Global) => None,
        (CapabilityEffect::Internal, _) => Some(ToolAccessClass::Internal),
    }
}

fn builtin_tool_specs(include_agent_name_tool: bool) -> Result<Vec<ToolSpec>, ToolContractError> {
    let mut specs = vec![search_memory_tool_spec()?, task_inspect_tool_spec()?];
    if include_agent_name_tool {
        specs.push(update_own_name_tool_spec()?);
    }
    specs.push(artifact_create_local_file_tool_spec()?);
    Ok(specs)
}

fn render_availability_notice(notice: &CapabilityAvailabilityNotice) -> String {
    let capability = notice
        .capability
        .as_ref()
        .map_or("capability", ToolName::as_str);
    let status = match notice.status {
        CapabilityAvailabilityStatus::Unavailable => "unavailable",
        CapabilityAvailabilityStatus::AuthenticationRequired => "authentication_required",
        CapabilityAvailabilityStatus::Disabled => "disabled",
    };
    format!("- unavailable_capability\t{capability}\tstatus={status}")
}

#[cfg(feature = "local-model-evals")]
pub(crate) fn prompt_rows(tools: &[ToolSpec]) -> Vec<String> {
    tools
        .iter()
        .map(|tool| format!("- builtin\t{}\t{}", tool.name, tool.description))
        .collect()
}

fn catalog_prompt_rows(
    bindings: &CapabilityCatalogSnapshot,
    prompt_kinds: &BTreeMap<String, ModelToolPromptKind>,
    policy: &ToolPolicy,
    transport: ProviderToolTransport,
) -> Vec<String> {
    let strict_policy = policy.strict_for_dispatch();
    bindings
        .iter()
        .filter(|binding| strict_policy.allows_tool(binding.spec().name.as_str()))
        .map(|binding| {
            let spec = binding.spec();
            let kind = match prompt_kinds.get(spec.name.as_str()) {
                Some(ModelToolPromptKind::Builtin) => "builtin",
                Some(ModelToolPromptKind::Web) => "web",
                Some(ModelToolPromptKind::Capability) => "capability",
                None => "capability",
            };
            match transport {
                ProviderToolTransport::NoemaEnvelope => format!(
                    "- {kind}\t{}\t{}\tinput_schema={}",
                    spec.name,
                    spec.description,
                    spec.input_schema.as_value()
                ),
                ProviderToolTransport::Native => {
                    format!("- {kind}\t{}\t{}", spec.name, spec.description)
                }
                ProviderToolTransport::None => String::new(),
            }
        })
        .filter(|row| !row.is_empty())
        .collect()
}

#[derive(Debug, Clone, Copy)]
enum BindingPersistence {
    Redacted,
    WebFetch,
    Artifact,
}

fn runtime_binding(
    spec: ToolSpec,
    class: ToolAccessClass,
    persistence: BindingPersistence,
) -> CapabilityBinding {
    let canonical_name = spec.name.as_str().to_string();
    let access = match class {
        ToolAccessClass::ReadOnly => CapabilityAccess {
            effect: CapabilityEffect::ReadOnly,
            scope: CapabilityScope::Global,
        },
        ToolAccessClass::TaskOwnedWrite
        | ToolAccessClass::ExecutorTerminal
        | ToolAccessClass::ReviewerTerminal => CapabilityAccess {
            effect: CapabilityEffect::Mutating,
            scope: CapabilityScope::ExecutionOwned,
        },
        ToolAccessClass::ConversationWrite => CapabilityAccess {
            effect: CapabilityEffect::Mutating,
            scope: CapabilityScope::ConversationOwned,
        },
        ToolAccessClass::Internal => CapabilityAccess {
            effect: CapabilityEffect::Internal,
            scope: CapabilityScope::Global,
        },
    };
    let sanitizer: Arc<dyn noema_capabilities::PayloadSanitizer> = match persistence {
        BindingPersistence::Redacted => Arc::new(RedactingPayloadSanitizer),
        BindingPersistence::WebFetch => Arc::new(WebFetchPayloadSanitizer),
        BindingPersistence::Artifact => Arc::new(ArtifactPayloadSanitizer),
    };
    CapabilityBinding::new(
        spec,
        CapabilityTarget::new(
            InvokerKey::new("runtime-execution"),
            noema_capabilities::OperationToken::new(canonical_name),
        ),
        access,
        sanitizer,
    )
}

fn add_binding(
    catalog: &mut CapabilityCatalogBuilder,
    binding: CapabilityBinding,
) -> Result<(), ToolContractError> {
    catalog.add(binding).map_err(|_| {
        ToolContractError::InvalidSchema("duplicate canonical capability binding".to_string())
    })
}

fn store_tool_error(_error: noema_store::StoreError) -> ToolContractError {
    ToolContractError::InvalidSchema("capability catalog is unavailable".to_string())
}

fn binding_source_tool_error(_error: CapabilityBindingSourceError) -> ToolContractError {
    ToolContractError::InvalidSchema("capability catalog is unavailable".to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use super::*;
    use noema_capabilities::{
        CapabilityBindingSource, CapabilityCatalogResult, CapabilityFuture, OmitPayloadSanitizer,
    };
    use noema_providers::{
        ProviderToolCapabilities, ProviderToolSchemaDialect, ProviderToolTransport,
    };
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
        ) -> CapabilityFuture<'_, Result<CapabilityCatalogResult, CapabilityBindingSourceError>>
        {
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
                    capability: Some(
                        ToolName::new("mcp.mcp:docs.read").expect("MCP test tool name"),
                    ),
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
        let NoemaToolChoice::Allowed(allowed) =
            tools.allowed_tool_choice(NoemaAllowedToolsMode::Auto)
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

        let pool_ids = delegation.input_schema.as_value()["properties"]
            ["executor_model_pool_entry_id"]["enum"]
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
}
