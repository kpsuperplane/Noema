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

#[cfg(feature = "eval-support")]
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
#[path = "model_tools/tests.rs"]
mod tests;
