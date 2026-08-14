use noema_store::NoemaStore;

#[cfg(test)]
use super::presentation_tools::{PRESENT_A2UI_TOOL, PRESENT_MULTIPLE_CHOICE_TOOL};
use super::presentation_tools::{present_a2ui_tool_spec, present_multiple_choice_tool_spec};

use crate::{
    agent_execution::{ExecutionRole, ToolAccessClass, ToolPolicy},
    daemon::{
        agent_name_tool::update_own_name_tool_spec,
        artifact_tool::artifact_create_local_file_tool_spec,
        task_artifact_tool::{TASK_READ_ARTIFACT_TOOL, task_read_artifact_tool_spec},
        task_run_context::TaskTerminalContract,
        task_submission_evidence_tool::{
            TASK_READ_SUBMISSION_EVIDENCE_TOOL, task_read_submission_evidence_tool_spec,
        },
        task_tool::{
            TASK_ANSWER_TOOL, TASK_CANCEL_TOOL, TASK_LIST_TOOL, TASK_REPORT_BLOCKED_TOOL,
            TASK_SUBMIT_PLAN_TOOL, TASK_SUBMIT_RESULT_TOOL, TASK_SUBMIT_REVIEW_TOOL,
            primary_task_tool_specs, task_list_scoped_tool_spec, task_report_blocked_tool_spec,
            task_submit_plan_tool_spec, task_submit_result_tool_spec, task_submit_review_tool_spec,
        },
    },
    search::tool::web_search_tool_spec,
    web_fetch::tool::web_fetch_tool_spec,
};
use noema_capabilities::{
    ArtifactPayloadSanitizer, CapabilityAvailabilityNotice, CapabilityAvailabilityStatus,
    CapabilityBinding, CapabilityBindingSourceError, CapabilityBindingSourceHandle,
    CapabilityCatalogBuilder, CapabilityCatalogSnapshot, CapabilityExecutionDecision,
    CapabilityScope, CapabilityTarget, CapabilityToolBehavior, InvokerKey,
    RedactingPayloadSanitizer, ToolContractError, ToolName, ToolSpec, WebBrowsePayloadSanitizer,
    WebFetchPayloadSanitizer, tool_enablement_name,
};
use noema_memory::{native_search_memory_tool_spec, read_memory_page_tool_spec};
use noema_providers::{
    NoemaAllowedTools, NoemaAllowedToolsMode, NoemaToolChoice, ProviderTool,
    ProviderToolCapabilities, ProviderToolTransport, expose_provider_tools,
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
    /// Exact request-scoped identities shared by prompt visibility and the
    /// provider's tool channel.
    pub(in crate::daemon) provider_tools: Vec<ProviderTool>,
    /// Whether this provider can execute hosted live-web search for the role.
    pub(in crate::daemon) hosted_web_search: bool,
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
        None,
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
    terminal_contract: Option<&TaskTerminalContract>,
) -> Result<ModelTools, ToolContractError> {
    let transport = capabilities.tool_transport;
    let capability_catalog = capability_bindings
        .catalog()
        .await
        .map_err(binding_source_tool_error)?;
    let unavailable_rows = capability_catalog
        .availability_notices
        .iter()
        .map(|notice| {
            render_availability_notice(
                notice,
                &capability_catalog.snapshot,
                transport != ProviderToolTransport::None,
            )
        })
        .collect();
    if transport == ProviderToolTransport::None {
        return Ok(ModelTools {
            transport,
            bindings: CapabilityCatalogSnapshot::default(),
            provider_tools: Vec::new(),
            hosted_web_search: false,
            prompt_rows: Vec::new(),
            unavailable_rows,
            prompt_kinds: BTreeMap::new(),
            tool_policy: ToolPolicy::for_role(role),
        });
    }

    let builtin_tools = role_builtin_tool_specs(role, include_agent_name_tool, terminal_contract)?;
    let web_search_tool = web_search_tool_spec()?;
    let web_fetch_tool = web_fetch_tool_spec()?;
    let web_browse_tools = noema_capabilities::web::browse::tool_specs()?;
    let mut tool_policy = ToolPolicy::for_role(role);
    let mut declared_builtin_tools = Vec::new();
    for tool in builtin_tools {
        let class = builtin_tool_access_class(role, tool.name.as_str());
        if tool_policy.declare_tool(tool.name.as_str(), class) {
            declared_builtin_tools.push((tool, class));
        }
    }
    let web_tools_allowed = !matches!(
        role,
        ExecutionRole::TaskPlanner | ExecutionRole::TaskReviewer
    );
    let web_provider_override = if capabilities.hosted_web_provider_name.is_some() {
        super::web_tools::web_provider_override_exists(store)
            .await
            .map_err(|_| {
                ToolContractError::InvalidSchema(
                    "web provider selection is unavailable".to_string(),
                )
            })?
    } else {
        false
    };
    let hosted_web_search = web_tools_allowed
        && capabilities.hosted_web_provider_name.is_some()
        && !web_provider_override
        && tool_policy.allows_class(web_tool_access_class(web_search_tool.name.as_str()));
    let declared_web_tools = if !web_tools_allowed {
        Vec::new()
    } else {
        let ordinary_web_tools = (!hosted_web_search)
            .then_some([web_search_tool, web_fetch_tool])
            .into_iter()
            .flatten();
        ordinary_web_tools
            .chain(web_browse_tools)
            .filter(|tool| {
                tool_policy.declare_tool(
                    tool.name.as_str(),
                    web_tool_access_class(tool.name.as_str()),
                )
            })
            .collect::<Vec<_>>()
    };

    let mut catalog = CapabilityCatalogBuilder::new();
    let mut prompt_kinds = BTreeMap::new();
    for (tool, class) in declared_builtin_tools {
        prompt_kinds.insert(tool.name.as_str().to_string(), ModelToolPromptKind::Builtin);
        let persistence = if matches!(tool.name.as_str(), "search_memory" | "read_memory_page") {
            BindingPersistence::Memory
        } else if tool.name.as_str() == "artifact.create_local_file" {
            BindingPersistence::Artifact
        } else {
            BindingPersistence::Redacted
        };
        add_binding(&mut catalog, runtime_binding(tool, class, persistence)?)?;
    }
    for tool in declared_web_tools {
        prompt_kinds.insert(tool.name.as_str().to_string(), ModelToolPromptKind::Web);
        add_binding(&mut catalog, native_web_binding(store, tool).await?)?;
    }
    let unavailable_capabilities = capability_catalog
        .availability_notices
        .iter()
        .filter_map(|notice| notice.capability.as_ref().map(ToolName::as_str))
        .collect::<std::collections::HashSet<_>>();
    for binding in capability_catalog.snapshot.iter() {
        if role == ExecutionRole::TaskPlanner {
            continue;
        }
        let callable = !unavailable_capabilities.contains(binding.spec().name.as_str());
        let access_class = capability_access_class(binding);
        let background_connector_proposal = is_background_connector_proposal(role, binding);
        if !tool_policy.allows_class(access_class) && !background_connector_proposal {
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
        if background_connector_proposal {
            tool_policy.allow_tool_name(spec.name.as_str());
        } else {
            tool_policy.declare_tool(spec.name.as_str(), access_class);
        }
    }

    let bindings = catalog.build();
    let provider_tools = expose_provider_tools(
        bindings.provider_specs(),
        transport,
        capabilities.schema_dialect,
    );
    let mut prompt_rows = catalog_prompt_rows(
        &provider_tools,
        &bindings,
        &prompt_kinds,
        &tool_policy,
        transport,
    );
    if hosted_web_search {
        prompt_rows.push(
            "- provider_native\tweb_search\tSearch the live web through the active model provider"
                .to_string(),
        );
    }

    Ok(ModelTools {
        transport,
        bindings,
        provider_tools,
        hosted_web_search,
        prompt_rows,
        unavailable_rows,
        prompt_kinds,
        tool_policy,
    })
}

/// Allow only the adapter-owned write that publishes an unreviewed proposal.
fn is_background_connector_proposal(role: ExecutionRole, binding: &CapabilityBinding) -> bool {
    role == ExecutionRole::TaskExecutor
        && binding.spec().name.as_str() == "adapter.propose_definition"
        && binding.target().invoker_key().as_str() == "adapter_json_v1"
        && binding.target().operation_token().as_str() == "adapter-setup-v1:propose-definition"
}

fn role_builtin_tool_specs(
    role: ExecutionRole,
    include_agent_name_tool: bool,
    terminal_contract: Option<&TaskTerminalContract>,
) -> Result<Vec<ToolSpec>, ToolContractError> {
    let criterion_ids = terminal_contract
        .map(|contract| contract.criterion_ids.as_slice())
        .unwrap_or_default();
    let mut tools = match role {
        ExecutionRole::TaskPlanner => {
            vec![
                task_submit_plan_tool_spec()?,
                task_report_blocked_tool_spec()?,
            ]
        }
        ExecutionRole::TaskExecutor => {
            vec![
                task_submit_result_tool_spec(criterion_ids)?,
                task_report_blocked_tool_spec()?,
                task_list_scoped_tool_spec()?,
            ]
        }
        ExecutionRole::TaskReviewer => vec![task_submit_review_tool_spec(criterion_ids)?],
        ExecutionRole::PrimaryConversation => Vec::new(),
    };
    if role == ExecutionRole::TaskReviewer {
        tools.push(task_read_submission_evidence_tool_spec()?);
    }
    if role != ExecutionRole::TaskPlanner {
        tools.extend(builtin_tool_specs(include_agent_name_tool)?);
    }
    if role == ExecutionRole::PrimaryConversation {
        tools.push(present_multiple_choice_tool_spec()?);
        tools.push(present_a2ui_tool_spec()?);
        tools.extend(primary_task_tool_specs()?);
    } else if role == ExecutionRole::TaskExecutor
        || role == ExecutionRole::TaskReviewer
            && terminal_contract.is_some_and(|contract| contract.has_submission_artifacts)
    {
        tools.push(task_read_artifact_tool_spec()?);
    }
    Ok(tools)
}

#[cfg(feature = "eval-support")]
pub(crate) fn task_role_builtin_tool_specs(
    role: ExecutionRole,
    terminal_contract: &TaskTerminalContract,
) -> Result<Vec<ToolSpec>, ToolContractError> {
    let mut policy = ToolPolicy::for_role(role);
    Ok(
        role_builtin_tool_specs(role, false, Some(terminal_contract))?
            .into_iter()
            .filter(|tool| {
                policy.declare_tool(
                    tool.name.as_str(),
                    builtin_tool_access_class(role, tool.name.as_str()),
                )
            })
            .collect(),
    )
}

impl ModelTools {
    pub(in crate::daemon) fn empty(role: ExecutionRole) -> Self {
        Self {
            transport: ProviderToolTransport::None,
            bindings: CapabilityCatalogSnapshot::default(),
            provider_tools: Vec::new(),
            hosted_web_search: false,
            prompt_rows: Vec::new(),
            unavailable_rows: Vec::new(),
            prompt_kinds: BTreeMap::new(),
            tool_policy: ToolPolicy::for_role(role),
        }
    }

    pub(in crate::daemon) fn has_callable_tools(&self) -> bool {
        self.hosted_web_search
            || self
                .bindings
                .iter()
                .any(|binding| self.tool_policy.allows_tool(binding.spec().name.as_str()))
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
            .intersect_allowed_names(&continuation.tool_policy);
        let hosted_web_search = initial.hosted_web_search && continuation.hosted_web_search;
        let mut prompt_rows = catalog_prompt_rows(
            &initial.provider_tools,
            &initial.bindings,
            &initial.prompt_kinds,
            &tool_policy,
            initial.transport,
        );
        if hosted_web_search {
            prompt_rows.push(
                "- provider_native\tweb_search\tSearch the live web through the active model provider"
                    .to_string(),
            );
        }
        Self {
            transport: initial.transport,
            bindings: initial.bindings.clone(),
            provider_tools: initial.provider_tools.clone(),
            hosted_web_search,
            prompt_rows,
            unavailable_rows: initial.unavailable_rows.clone(),
            prompt_kinds: initial.prompt_kinds.clone(),
            tool_policy,
        }
    }

    pub(in crate::daemon) fn callable_tool_names(&self) -> Vec<String> {
        let mut names = self
            .provider_tools
            .iter()
            .filter(|tool| {
                self.tool_policy
                    .allows_tool(tool.canonical_spec().name.as_str())
            })
            .map(|tool| tool.exposed_name().to_string())
            .collect::<Vec<_>>();
        if self.hosted_web_search {
            names.push("web_search".to_string());
        }
        names
    }

    pub(in crate::daemon) const fn hosted_web_search(&self) -> bool {
        self.hosted_web_search
    }

    pub(in crate::daemon) fn provider_tools(&self) -> Vec<ProviderTool> {
        if self.transport != ProviderToolTransport::None {
            self.provider_tools.clone()
        } else {
            Vec::new()
        }
    }

    pub(in crate::daemon) fn policy_filtered_provider_tools(&self) -> Vec<ProviderTool> {
        if self.transport == ProviderToolTransport::None {
            return Vec::new();
        }
        self.provider_tools
            .iter()
            .filter(|tool| {
                self.tool_policy
                    .allows_tool(tool.canonical_spec().name.as_str())
            })
            .cloned()
            .collect()
    }

    pub(in crate::daemon) fn provider_tools_for_specs(
        &self,
        specs: &[ToolSpec],
    ) -> Vec<ProviderTool> {
        let names = specs
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<std::collections::HashSet<_>>();
        self.provider_tools
            .iter()
            .filter(|tool| names.contains(tool.canonical_spec().name.as_str()))
            .cloned()
            .collect()
    }

    pub(in crate::daemon) fn allowed_tool_choice(
        &self,
        mode: NoemaAllowedToolsMode,
    ) -> NoemaToolChoice {
        let tools: Vec<ToolName> = self
            .provider_tools
            .iter()
            .filter(|tool| {
                self.tool_policy
                    .allows_tool(tool.canonical_spec().name.as_str())
            })
            .map(|tool| tool.canonical_spec().name.clone())
            .collect();
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
        "search_memory"
        | "read_memory_page"
        | TASK_LIST_TOOL
        | TASK_READ_ARTIFACT_TOOL
        | TASK_READ_SUBMISSION_EVIDENCE_TOOL => ToolAccessClass::ReadOnly,
        "artifact.create_local_file"
            if matches!(
                role,
                ExecutionRole::TaskPlanner | ExecutionRole::TaskReviewer
            ) =>
        {
            ToolAccessClass::ConversationWrite
        }
        "artifact.create_local_file" if role == ExecutionRole::TaskExecutor => {
            ToolAccessClass::TaskOwnedWrite
        }
        "artifact.create_local_file" => ToolAccessClass::ConversationWrite,
        // Renaming the primary identity is a foreground-only control action.
        "update_own_name" | TASK_ANSWER_TOOL | TASK_CANCEL_TOOL => ToolAccessClass::Internal,
        TASK_SUBMIT_PLAN_TOOL | TASK_REPORT_BLOCKED_TOOL => ToolAccessClass::ExecutorTerminal,
        TASK_SUBMIT_RESULT_TOOL => ToolAccessClass::ExecutorTerminal,
        TASK_SUBMIT_REVIEW_TOOL => ToolAccessClass::ReviewerTerminal,
        _ => ToolAccessClass::Internal,
    }
}

fn web_tool_access_class(name: &str) -> ToolAccessClass {
    match name {
        // Search only sends a query to the configured, trusted search provider;
        // it is a routine retrieval and must not create an approval prompt.
        noema_capabilities::web::search::WEB_SEARCH_TOOL => ToolAccessClass::ReadOnly,
        noema_capabilities::web::browse::WEB_BROWSE_SNAPSHOT_TOOL
        | noema_capabilities::web::browse::WEB_BROWSE_WAIT_TOOL
        | noema_capabilities::web::browse::WEB_BROWSE_CLOSE_TOOL => ToolAccessClass::ReadOnly,
        // Fetch and browser open-world actions stay behind the
        // governed-action gateway (with its existing observed-URL admission).
        _ => ToolAccessClass::ExternalTool,
    }
}

fn capability_access_class(binding: &CapabilityBinding) -> ToolAccessClass {
    if binding.destination().is_some() {
        return ToolAccessClass::ExternalTool;
    }
    if binding.behavior().read_only {
        return ToolAccessClass::ReadOnly;
    }
    match binding.scope() {
        CapabilityScope::ExecutionOwned => ToolAccessClass::TaskOwnedWrite,
        CapabilityScope::ConversationOwned => ToolAccessClass::ConversationWrite,
        CapabilityScope::Global => ToolAccessClass::Internal,
    }
}

fn builtin_tool_specs(include_agent_name_tool: bool) -> Result<Vec<ToolSpec>, ToolContractError> {
    let mut specs = vec![
        read_memory_page_tool_spec()?,
        native_search_memory_tool_spec()?,
    ];
    if include_agent_name_tool {
        specs.push(update_own_name_tool_spec()?);
    }
    specs.push(artifact_create_local_file_tool_spec()?);
    Ok(specs)
}

fn render_availability_notice(
    notice: &CapabilityAvailabilityNotice,
    catalog: &CapabilityCatalogSnapshot,
    enablement_supported: bool,
) -> String {
    let capability = notice
        .capability
        .as_ref()
        .map_or("capability", ToolName::as_str);
    let status = match notice.status {
        CapabilityAvailabilityStatus::Unavailable => "unavailable",
        CapabilityAvailabilityStatus::AuthenticationRequired => "authentication_required",
        CapabilityAvailabilityStatus::Disabled => "disabled",
    };
    let enablement = notice
        .capability
        .as_ref()
        .and_then(|name| tool_enablement_name(name).ok())
        .filter(|name| enablement_supported && catalog.resolve(name.as_str()).is_some())
        .map(|name| format!("\tenable_with={name}"))
        .unwrap_or_default();
    format!("- unavailable_capability\t{capability}\tstatus={status}{enablement}")
}

#[cfg(feature = "eval-support")]
pub(crate) fn prompt_rows(tools: &[ToolSpec]) -> Vec<String> {
    tools
        .iter()
        .map(|tool| format!("- builtin\t{}\t{}", tool.name, tool.description))
        .collect()
}

fn catalog_prompt_rows(
    tools: &[ProviderTool],
    bindings: &CapabilityCatalogSnapshot,
    prompt_kinds: &BTreeMap<String, ModelToolPromptKind>,
    policy: &ToolPolicy,
    transport: ProviderToolTransport,
) -> Vec<String> {
    let visible = tools
        .iter()
        .filter(|tool| policy.allows_tool(tool.canonical_spec().name.as_str()))
        .collect::<Vec<_>>();
    let services = visible
        .iter()
        .filter_map(|tool| bindings.resolve(tool.canonical_spec().name.as_str()))
        .filter_map(|binding| Some((binding.destination()?, binding.service_context()?)))
        .map(|(destination, context)| {
            (
                (
                    destination.service_id().to_string(),
                    destination.connection_id().to_string(),
                    destination.account_id().map(str::to_string),
                    destination.revision().to_string(),
                ),
                context,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut rows = services
        .into_iter()
        .map(|((_, connection_id, _, _), context)| {
            let name = serde_json::to_string(context.display_name()).expect("string serialization");
            let mut row = format!("- service\t{connection_id}\tname={name}");
            if let Some(connection_label) = context.connection_label() {
                row.push_str("\tconnection_label=");
                row.push_str(
                    &serde_json::to_string(connection_label).expect("string serialization"),
                );
            }
            if let Some(description) = context.description() {
                row.push_str("\tdescription=");
                row.push_str(&serde_json::to_string(description).expect("string serialization"));
            }
            row
        })
        .collect::<Vec<_>>();
    rows.extend(
        visible
            .into_iter()
            .map(|tool| {
                let spec = tool.canonical_spec();
                let kind = match prompt_kinds.get(spec.name.as_str()) {
                    Some(ModelToolPromptKind::Builtin) => "builtin",
                    Some(ModelToolPromptKind::Web) => "web",
                    Some(ModelToolPromptKind::Capability) => "capability",
                    None => "capability",
                };
                let service = bindings.resolve(spec.name.as_str()).and_then(|binding| {
                    binding.service_context()?;
                    binding
                        .destination()
                        .map(|destination| destination.connection_id())
                });
                let service = service
                    .map(|service| format!("\tservice={service}"))
                    .unwrap_or_default();
                match transport {
                    ProviderToolTransport::Native => {
                        format!(
                            "- {kind}\t{}{service}\t{}",
                            tool.exposed_name(),
                            spec.description
                        )
                    }
                    ProviderToolTransport::None => String::new(),
                }
            })
            .filter(|row| !row.is_empty()),
    );
    rows
}

#[derive(Debug, Clone, Copy)]
enum BindingPersistence {
    Redacted,
    WebFetch,
    WebBrowse,
    Artifact,
    Memory,
}

#[derive(Debug, Default)]
struct NativeMemoryPayloadSanitizer;

impl noema_capabilities::PayloadSanitizer for NativeMemoryPayloadSanitizer {
    fn persist_arguments(&self, arguments: &serde_json::Value) -> Option<serde_json::Value> {
        Some(
            arguments
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| arguments.clone()),
        )
    }

    fn persist_output(&self, output: &serde_json::Value) -> Option<serde_json::Value> {
        if let Some(page) = output.get("page") {
            return Some(
                serde_json::json!({"page_ref": {"id": page.get("id"), "path": page.get("path"), "hash": page.get("hash")}}),
            );
        }
        if let Some(pages) = output.get("pages") {
            return Some(
                serde_json::json!({"pages": pages.as_array().map(|pages| pages.iter().map(|page| serde_json::json!({"id": page.get("id"), "path": page.get("path"), "hash": page.get("hash")})).collect::<Vec<_>>())}),
            );
        }
        Some(serde_json::json!({"memory_result": "omitted"}))
    }
}

fn runtime_binding(
    spec: ToolSpec,
    class: ToolAccessClass,
    persistence: BindingPersistence,
) -> Result<CapabilityBinding, ToolContractError> {
    let canonical_name = spec.name.as_str().to_string();
    let is_non_idempotent_browse_action = matches!(
        canonical_name.as_str(),
        noema_capabilities::web::browse::WEB_BROWSE_INTERACT_TOOL
            | noema_capabilities::web::browse::WEB_BROWSE_HISTORY_TOOL
    );
    let (mut behavior, execution_decision, scope) = match class {
        ToolAccessClass::ReadOnly => (
            CapabilityToolBehavior {
                read_only: true,
                idempotent: true,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::Global,
        ),
        ToolAccessClass::TaskOwnedWrite
        | ToolAccessClass::ExecutorTerminal
        | ToolAccessClass::ReviewerTerminal => (
            CapabilityToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::ExecutionOwned,
        ),
        ToolAccessClass::ConversationWrite => (
            CapabilityToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::ConversationOwned,
        ),
        ToolAccessClass::ExternalTool => (
            CapabilityToolBehavior {
                read_only: true,
                idempotent: true,
                destructive: false,
                open_world: true,
            },
            CapabilityExecutionDecision::LlmReview,
            CapabilityScope::Global,
        ),
        ToolAccessClass::Internal => (
            CapabilityToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: false,
            },
            CapabilityExecutionDecision::ExecuteImmediately,
            CapabilityScope::Global,
        ),
    };
    if is_non_idempotent_browse_action {
        behavior.read_only = false;
        behavior.idempotent = false;
    }
    let sanitizer: Arc<dyn noema_capabilities::PayloadSanitizer> = match persistence {
        BindingPersistence::Redacted => Arc::new(RedactingPayloadSanitizer),
        BindingPersistence::WebFetch => Arc::new(WebFetchPayloadSanitizer),
        BindingPersistence::WebBrowse => Arc::new(WebBrowsePayloadSanitizer),
        BindingPersistence::Artifact => Arc::new(ArtifactPayloadSanitizer),
        BindingPersistence::Memory => Arc::new(NativeMemoryPayloadSanitizer),
    };
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .should_ignore_unknown_formats(false)
        .build(spec.input_schema.as_value())
        .map_err(|error| ToolContractError::InvalidSchema(error.to_string()))?;
    let input_check: Arc<dyn noema_capabilities::ToolInputCheck> =
        Arc::new(move |arguments: &serde_json::Value| validator.is_valid(arguments));
    Ok(CapabilityBinding::new(
        spec,
        CapabilityTarget::new(
            InvokerKey::new("runtime-execution"),
            noema_capabilities::OperationToken::new(canonical_name),
        ),
        behavior,
        execution_decision,
        scope,
        input_check,
        sanitizer,
    ))
}

pub(super) async fn native_web_binding(
    store: &NoemaStore,
    spec: ToolSpec,
) -> Result<CapabilityBinding, ToolContractError> {
    let persistence = match spec.name.as_str() {
        noema_capabilities::web::fetch::WEB_FETCH_TOOL => BindingPersistence::WebFetch,
        name if name.starts_with("web.browse.") => BindingPersistence::WebBrowse,
        _ => BindingPersistence::Redacted,
    };
    let class = web_tool_access_class(spec.name.as_str());
    let binding = runtime_binding(spec, class, persistence)?;
    let destination =
        super::web_tools::resolve_web_destination(store, binding.spec().name.as_str())
            .await
            .map_err(|_| {
                ToolContractError::InvalidSchema(
                    "web capability destination is unavailable".to_string(),
                )
            })?;
    Ok(binding.with_destination(destination))
}

fn add_binding(
    catalog: &mut CapabilityCatalogBuilder,
    binding: CapabilityBinding,
) -> Result<(), ToolContractError> {
    catalog.add(binding).map_err(|_| {
        ToolContractError::InvalidSchema("duplicate canonical capability binding".to_string())
    })
}

fn binding_source_tool_error(_error: CapabilityBindingSourceError) -> ToolContractError {
    ToolContractError::InvalidSchema("capability catalog is unavailable".to_string())
}

#[cfg(test)]
#[path = "model_tools/tests.rs"]
mod tests;
