use std::collections::HashSet;

use async_graphql::{Enum, InputObject, Result, SimpleObject};
use noema_providers::{
    LocalModelInstallationRecord, LocalModelInstallationStatus, ModelPreferenceSelection,
    NoemaModelUseCase, ProviderAccountRecord, ProviderAccountStatus, ProviderKind,
    ProviderModelProfile, ReasoningEffort, noema_model_recommendation,
};
use noema_tasks::TASK_EXECUTOR_AGENT_ID;
use serde_json::Value;

use noema_store::{
    AgentRecord, AgentRuntimePreferenceRecord, AuxiliaryModelTask, NewAgentRuntimePreference,
    NewAuxiliaryModelPreference,
};

use super::{errors::graphql_error, schema::GraphqlState};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ReasoningEffort")]
pub enum GraphqlReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

graphql_enum_bidi!(ReasoningEffort => GraphqlReasoningEffort {
    None => None,
    Minimal => Minimal,
    Low => Low,
    Medium => Medium,
    High => High,
    XHigh => Xhigh,
});

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ModelPreferenceSelectionMode")]
pub enum GraphqlModelPreferenceSelectionMode {
    NoemaRecommended,
    ExplicitProfile,
}

impl From<&ModelPreferenceSelection> for GraphqlModelPreferenceSelectionMode {
    fn from(value: &ModelPreferenceSelection) -> Self {
        match value {
            ModelPreferenceSelection::NoemaRecommended => Self::NoemaRecommended,
            ModelPreferenceSelection::ExplicitProfile { .. } => Self::ExplicitProfile,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "NoemaModelUseCase")]
pub enum GraphqlNoemaModelUseCase {
    Primary,
    TaskSimple,
    TaskMedium,
    TaskDifficult,
    TaskReviewer,
    WebFetchSummarizer,
    ToolProgressAudit,
    ActionReviewer,
    MemoryConsolidation,
}

graphql_enum_from!(NoemaModelUseCase => GraphqlNoemaModelUseCase {
    Primary => Primary,
    TaskSimple => TaskSimple,
    TaskMedium => TaskMedium,
    TaskDifficult => TaskDifficult,
    TaskReviewer => TaskReviewer,
    WebFetchSummarizer => WebFetchSummarizer,
    ToolProgressAudit => ToolProgressAudit,
    ActionReviewer => ActionReviewer,
    MemoryConsolidation => MemoryConsolidation,
});

/// Agent model preference safe to expose in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AgentModelPreference")]
pub struct GraphqlAgentModelPreference {
    /// Provider kind selected for this agent.
    pub provider_kind: String,
    /// Provider account id selected for this agent.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: Option<String>,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    /// Whether Noema or the human chooses the concrete model.
    pub selection_mode: GraphqlModelPreferenceSelectionMode,
}

/// Noema's current concrete recommendation for one workload.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AgentModelRecommendation")]
pub struct GraphqlAgentModelRecommendation {
    pub use_case: GraphqlNoemaModelUseCase,
    pub model_profile: String,
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    pub disabled_reason: Option<String>,
}

/// One selectable model/profile.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AgentModelProfileOption")]
pub struct GraphqlAgentModelProfileOption {
    /// Stable profile or model id.
    pub id: String,
    /// User-facing label.
    pub label: String,
    /// Reasoning efforts available for this profile.
    pub reasoning_efforts: Vec<GraphqlReasoningEffort>,
    /// Default reasoning effort for this profile, when advertised by metadata.
    pub default_reasoning_effort: Option<GraphqlReasoningEffort>,
    /// Why this option is disabled, when unavailable.
    pub disabled_reason: Option<String>,
}

/// One selectable provider account and its profiles.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AgentModelProviderOption")]
pub struct GraphqlAgentModelProviderOption {
    /// Provider kind.
    pub provider_kind: String,
    /// Provider account id.
    pub provider_account_id: String,
    /// User-facing provider display name.
    pub provider_display_name: String,
    /// Provider account status.
    pub status: super::onboarding::GraphqlProviderAccountStatus,
    /// Available profiles or model ids.
    pub profiles: Vec<GraphqlAgentModelProfileOption>,
    /// Current product recommendations available through this account.
    pub recommendations: Vec<GraphqlAgentModelRecommendation>,
    /// Why this provider is disabled, when unavailable.
    pub disabled_reason: Option<String>,
}

/// Input for saving an agent model preference.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveAgentModelPreferenceInput")]
pub struct GraphqlSaveAgentModelPreferenceInput {
    /// Agent to update.
    pub agent_id: String,
    /// Provider account id to use.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub selection_mode: GraphqlModelPreferenceSelectionMode,
    /// Provider-specific model id or profile id for explicit selections.
    pub model_profile: Option<String>,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}

/// Agent metadata safe to expose in read-only Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "Agent")]
pub struct GraphqlAgent {
    /// Durable concrete agent id.
    pub agent_id: String,
    /// Optional human-visible agent name.
    pub display_name: Option<String>,
    /// Whether this is Noema's built-in primary agent.
    pub is_primary: bool,
    /// Current model preference, when configured.
    pub model_preference: Option<GraphqlAgentModelPreference>,
    /// Provider/profile options available to this agent.
    pub model_options: Vec<GraphqlAgentModelProviderOption>,
}

impl GraphqlAgent {
    fn from_parts(
        agent: AgentRecord,
        preference: Option<AgentRuntimePreferenceRecord>,
        model_options: &[GraphqlAgentModelProviderOption],
    ) -> Self {
        let is_primary = agent.agent_id == "agent:primary";
        Self {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
            is_primary,
            model_preference: preference.map(|preference| {
                graphql_agent_preference(
                    preference.provider_kind,
                    preference.provider_account_id,
                    preference.selection,
                )
            }),
            model_options: model_options.to_vec(),
        }
    }
}

fn graphql_agent_preference(
    provider_kind: String,
    provider_account_id: String,
    selection: ModelPreferenceSelection,
) -> GraphqlAgentModelPreference {
    let (model_profile, reasoning_effort) = match &selection {
        ModelPreferenceSelection::NoemaRecommended => (None, None),
        ModelPreferenceSelection::ExplicitProfile {
            model_profile,
            reasoning_effort,
        } => (Some(model_profile.clone()), *reasoning_effort),
    };
    GraphqlAgentModelPreference {
        provider_kind,
        provider_account_id,
        model_profile,
        reasoning_effort: reasoning_effort.map(Into::into),
        selection_mode: (&selection).into(),
    }
}

pub(super) async fn agents(state: &GraphqlState) -> Result<Vec<GraphqlAgent>> {
    let store = state.store()?;
    let agents = store.list_agents().await.map_err(graphql_error)?;
    let accounts = active_default_model_accounts(state).await?;
    let model_options = model_options_from_accounts(store, &accounts).await?;
    let mut output = Vec::with_capacity(agents.len());
    for agent in agents {
        let preference = store
            .get_agent_runtime_preference(&agent.agent_id)
            .await
            .map_err(graphql_error)?;
        output.push(GraphqlAgent::from_parts(agent, preference, &model_options));
    }
    Ok(output)
}

pub(super) async fn active_default_model_accounts(
    state: &GraphqlState,
) -> Result<Vec<ProviderAccountRecord>> {
    let operations = state.provider_account_operations()?;
    let accounts = operations.active_accounts().await.map_err(graphql_error)?;
    for account in accounts.iter().filter(|account| account.is_default) {
        let _ = operations
            .refresh_model_catalog(&account.provider_account_id)
            .await;
    }
    Ok(operations
        .active_accounts()
        .await
        .map_err(graphql_error)?
        .into_iter()
        .filter(|account| account.is_default)
        .collect())
}

pub(super) async fn selectable_model_account(
    state: &GraphqlState,
    provider_account_id: &str,
) -> Result<ProviderAccountRecord> {
    let operations = state.provider_account_operations()?;
    require_default_account(
        operations.active_accounts().await.map_err(graphql_error)?,
        provider_account_id,
    )?;
    let _ = operations.refresh_model_catalog(provider_account_id).await;
    require_default_account(
        operations.active_accounts().await.map_err(graphql_error)?,
        provider_account_id,
    )
}

fn require_default_account(
    accounts: Vec<ProviderAccountRecord>,
    provider_account_id: &str,
) -> Result<ProviderAccountRecord> {
    let account = accounts
        .into_iter()
        .find(|account| account.provider_account_id == provider_account_id)
        .ok_or_else(|| async_graphql::Error::new("provider account not found"))?;
    if !account.is_default {
        return Err(async_graphql::Error::new(
            "provider account is not selectable",
        ));
    }
    Ok(account)
}

pub(super) async fn save_agent_model_preference(
    state: &GraphqlState,
    input: GraphqlSaveAgentModelPreferenceInput,
) -> Result<GraphqlAgentModelPreference> {
    if input.agent_id == TASK_EXECUTOR_AGENT_ID {
        return Err(async_graphql::Error::new(
            "Task Executor models are configured by complexity tier",
        ));
    }
    let store = state.store()?;
    let account = selectable_model_account(state, &input.provider_account_id).await?;
    if let Some(reason) = provider_disabled_reason(&account) {
        return Err(async_graphql::Error::new(reason));
    }
    let use_case = agent_model_use_case(&input.agent_id)?;
    let (selection, model_profile, reasoning_effort) = resolve_preference_input(
        store,
        &account,
        input.selection_mode,
        input.model_profile,
        input.reasoning_effort,
        use_case,
    )
    .await?;
    let ready_selection = super::provider_selection::prove_ready_selection(
        state,
        &account.provider_kind,
        &account.provider_account_id,
        &model_profile,
        reasoning_effort,
        "graphql_agent_runtime_preference",
    )
    .await?;
    let preference = NewAgentRuntimePreference {
        agent_id: input.agent_id,
        provider_kind: account.provider_kind,
        provider_account_id: account.provider_account_id,
        selection,
    };
    let saved = store
        .upsert_agent_runtime_preference_with_ready_selection(preference, &ready_selection)
        .await
        .map_err(graphql_error)?;
    Ok(graphql_agent_preference(
        saved.provider_kind,
        saved.provider_account_id,
        saved.selection,
    ))
}

fn option_from_account(
    account: &ProviderAccountRecord,
    local_installations: &[LocalModelInstallationRecord],
) -> GraphqlAgentModelProviderOption {
    let disabled_reason = provider_disabled_reason(account);
    let profiles = profiles_from_account(account, disabled_reason.as_deref(), local_installations);
    let recommendations = recommendations_from_account(account, &profiles);
    GraphqlAgentModelProviderOption {
        provider_kind: account.provider_kind.clone(),
        provider_account_id: account.provider_account_id.clone(),
        provider_display_name: account.display_name.clone(),
        status: account.status.into(),
        profiles,
        recommendations,
        disabled_reason,
    }
}

pub(super) fn recommendations_from_account(
    account: &ProviderAccountRecord,
    profiles: &[GraphqlAgentModelProfileOption],
) -> Vec<GraphqlAgentModelRecommendation> {
    let Ok(provider_kind) = account.provider_kind.parse::<ProviderKind>() else {
        return Vec::new();
    };
    NoemaModelUseCase::ALL
        .into_iter()
        .filter_map(|use_case| {
            let recommendation = noema_model_recommendation(provider_kind.clone(), use_case)?;
            let disabled_reason = provider_disabled_reason(account).or_else(|| {
                let profile = profiles
                    .iter()
                    .find(|profile| profile.id == recommendation.model_profile)?;
                profile.disabled_reason.clone().or_else(|| {
                    recommendation.reasoning_effort.and_then(|effort| {
                        (!profile.reasoning_efforts.contains(&effort.into()))
                            .then(|| "Recommended reasoning effort is unavailable.".to_string())
                    })
                })
            });
            let disabled_reason = disabled_reason.or_else(|| {
                (!profiles
                    .iter()
                    .any(|profile| profile.id == recommendation.model_profile))
                .then(|| "Recommended model is unavailable.".to_string())
            });
            Some(GraphqlAgentModelRecommendation {
                use_case: use_case.into(),
                model_profile: recommendation.model_profile.to_string(),
                reasoning_effort: recommendation.reasoning_effort.map(Into::into),
                disabled_reason,
            })
        })
        .collect()
}

pub(super) async fn model_options_from_accounts(
    store: &noema_store::NoemaStore,
    accounts: &[ProviderAccountRecord],
) -> Result<Vec<GraphqlAgentModelProviderOption>> {
    let local_installations = if accounts
        .iter()
        .any(|account| account.provider_kind == "local_models")
    {
        store
            .list_local_model_installations()
            .await
            .map_err(graphql_error)?
    } else {
        Vec::new()
    };
    Ok(accounts
        .iter()
        .map(|account| option_from_account(account, &local_installations))
        .collect())
}

pub(super) async fn selectable_profiles_from_account(
    store: &noema_store::NoemaStore,
    account: &ProviderAccountRecord,
) -> Result<Vec<GraphqlAgentModelProfileOption>> {
    let local_installations = if account.provider_kind == "local_models" {
        store
            .list_local_model_installations()
            .await
            .map_err(graphql_error)?
    } else {
        Vec::new()
    };
    Ok(profiles_from_account(account, None, &local_installations))
}

pub(super) struct AuxiliaryModelSettings {
    pub preference: Option<GraphqlAgentModelPreference>,
    pub options: Vec<GraphqlAgentModelProviderOption>,
}

pub(super) async fn auxiliary_model_settings(
    state: &GraphqlState,
    task: AuxiliaryModelTask,
) -> Result<AuxiliaryModelSettings> {
    let store = state.store()?;
    let accounts = active_default_model_accounts(state).await?;
    let preference = store
        .get_auxiliary_model_preference(task)
        .await
        .map_err(graphql_error)?
        .map(|preference| {
            graphql_agent_preference(
                preference.provider_kind,
                preference.provider_account_id,
                preference.selection,
            )
        });
    Ok(AuxiliaryModelSettings {
        preference,
        options: model_options_from_accounts(store, &accounts).await?,
    })
}

pub(super) async fn save_auxiliary_model_preference(
    state: &GraphqlState,
    task: AuxiliaryModelTask,
    provider_account_id: String,
    selection_mode: GraphqlModelPreferenceSelectionMode,
    model_profile: Option<String>,
    reasoning_effort: Option<GraphqlReasoningEffort>,
    provenance: &'static str,
) -> Result<GraphqlAgentModelPreference> {
    let store = state.store()?;
    let account = selectable_model_account(state, &provider_account_id).await?;
    if let Some(reason) = provider_disabled_reason(&account) {
        return Err(async_graphql::Error::new(reason));
    }
    let (selection, model_profile, reasoning_effort) = resolve_preference_input(
        store,
        &account,
        selection_mode,
        model_profile,
        reasoning_effort,
        auxiliary_model_use_case(task),
    )
    .await?;
    let ready_selection = super::provider_selection::prove_ready_selection(
        state,
        &account.provider_kind,
        &account.provider_account_id,
        &model_profile,
        reasoning_effort,
        provenance,
    )
    .await?;
    let saved = store
        .upsert_auxiliary_model_preference_with_ready_selection(
            NewAuxiliaryModelPreference {
                task,
                provider_kind: account.provider_kind,
                provider_account_id: account.provider_account_id,
                selection,
            },
            &ready_selection,
        )
        .await
        .map_err(graphql_error)?;
    Ok(graphql_agent_preference(
        saved.provider_kind,
        saved.provider_account_id,
        saved.selection,
    ))
}

pub(super) async fn resolve_preference_input(
    store: &noema_store::NoemaStore,
    account: &ProviderAccountRecord,
    selection_mode: GraphqlModelPreferenceSelectionMode,
    model_profile: Option<String>,
    reasoning_effort: Option<GraphqlReasoningEffort>,
    use_case: NoemaModelUseCase,
) -> Result<(ModelPreferenceSelection, String, Option<ReasoningEffort>)> {
    match selection_mode {
        GraphqlModelPreferenceSelectionMode::NoemaRecommended => {
            if model_profile.is_some() || reasoning_effort.is_some() {
                return Err(async_graphql::Error::new(
                    "Noema Recommended does not accept a model or reasoning effort",
                ));
            }
            let provider = account
                .provider_kind
                .parse::<ProviderKind>()
                .map_err(|_| async_graphql::Error::new("provider is unsupported"))?;
            let recommendation =
                noema_model_recommendation(provider, use_case).ok_or_else(|| {
                    async_graphql::Error::new(
                        "provider has no Noema recommendation for this use case",
                    )
                })?;
            let profiles = selectable_profiles_from_account(store, account).await?;
            let profile = require_selectable_profile(&profiles, recommendation.model_profile)?;
            let effort = match recommendation.reasoning_effort {
                Some(effort) => {
                    validate_reasoning_effort_for_profile(profile, Some(effort.into()))?
                }
                None => None,
            };
            Ok((
                ModelPreferenceSelection::NoemaRecommended,
                recommendation.model_profile.to_string(),
                effort,
            ))
        }
        GraphqlModelPreferenceSelectionMode::ExplicitProfile => {
            let model_profile = model_profile
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| async_graphql::Error::new("explicit model profile is required"))?;
            let profiles = selectable_profiles_from_account(store, account).await?;
            let profile = require_selectable_profile(&profiles, &model_profile)?;
            let effort = validate_reasoning_effort_for_profile(profile, reasoning_effort)?;
            Ok((
                ModelPreferenceSelection::ExplicitProfile {
                    model_profile: model_profile.clone(),
                    reasoning_effort: effort,
                },
                model_profile,
                effort,
            ))
        }
    }
}

fn agent_model_use_case(agent_id: &str) -> Result<NoemaModelUseCase> {
    match agent_id {
        "agent:primary" => Ok(NoemaModelUseCase::Primary),
        "agent:task-reviewer" => Ok(NoemaModelUseCase::TaskReviewer),
        _ => Err(async_graphql::Error::new(
            "agent has no configurable model recommendation",
        )),
    }
}

pub(super) const fn auxiliary_model_use_case(task: AuxiliaryModelTask) -> NoemaModelUseCase {
    match task {
        AuxiliaryModelTask::WebFetchSummarizer => NoemaModelUseCase::WebFetchSummarizer,
        AuxiliaryModelTask::ToolProgressAudit => NoemaModelUseCase::ToolProgressAudit,
        AuxiliaryModelTask::ActionReviewer => NoemaModelUseCase::ActionReviewer,
        AuxiliaryModelTask::MemoryConsolidation => NoemaModelUseCase::MemoryConsolidation,
    }
}

pub(super) fn provider_disabled_reason(account: &ProviderAccountRecord) -> Option<String> {
    match account.status {
        ProviderAccountStatus::Authenticated => None,
        ProviderAccountStatus::Unknown if account.provider_kind == "foundation_local" => {
            Some("Apple Foundation Models availability has not been checked.".to_string())
        }
        ProviderAccountStatus::Unknown => Some("Provider status has not been checked.".to_string()),
        ProviderAccountStatus::Checking => Some("Provider status is still checking.".to_string()),
        ProviderAccountStatus::Unauthenticated => {
            Some("Provider account is not authenticated.".to_string())
        }
        ProviderAccountStatus::Unavailable => Some(unavailable_provider_reason(account)),
    }
}

fn unavailable_provider_reason(account: &ProviderAccountRecord) -> String {
    match account.last_error_code.as_deref() {
        Some("unsupported_platform") => "Provider is unavailable on this platform.",
        Some("bridge_missing") => "Apple Foundation Models bridge is unavailable.",
        Some("foundation_models_unavailable") => {
            "Apple Foundation Models are unavailable on this machine."
        }
        _ => "Provider is unavailable on this machine.",
    }
    .to_string()
}

fn profiles_from_account(
    account: &ProviderAccountRecord,
    disabled_reason: Option<&str>,
    local_installations: &[LocalModelInstallationRecord],
) -> Vec<GraphqlAgentModelProfileOption> {
    // Local GGUF availability is installation state, never duplicated provider metadata.
    if account.provider_kind == "local_models" {
        return local_model_profile_options(local_installations, disabled_reason);
    }
    let metadata_profiles = metadata_profiles(
        &account.metadata,
        disabled_reason,
        matches!(
            account.provider_kind.as_str(),
            "openai" | "codex" | "openrouter"
        ),
    );
    if !metadata_profiles.is_empty() {
        return metadata_profiles;
    }
    match account.provider_kind.as_str() {
        "foundation_local" => profile_options(&[("default", "Default on-device")], disabled_reason),
        _ => Vec::new(),
    }
}

fn local_model_profile_options(
    installations: &[LocalModelInstallationRecord],
    provider_disabled_reason: Option<&str>,
) -> Vec<GraphqlAgentModelProfileOption> {
    let mut seen_model_ids = HashSet::new();
    installations
        .iter()
        .filter(|installation| installation.status == LocalModelInstallationStatus::Installed)
        .filter(|installation| seen_model_ids.insert(installation.model_id.as_str()))
        .map(|installation| GraphqlAgentModelProfileOption {
            id: installation.model_id.clone(),
            label: installation.display_name.clone(),
            reasoning_efforts: Vec::new(),
            default_reasoning_effort: None,
            disabled_reason: provider_disabled_reason
                .map(ToString::to_string)
                .or_else(|| {
                    (!installation.is_active).then(|| {
                        "Activate this model in Settings > Local models before assigning it."
                            .to_string()
                    })
                }),
        })
        .collect()
}

pub(super) fn require_selectable_profile<'a>(
    profiles: &'a [GraphqlAgentModelProfileOption],
    model_profile: &str,
) -> Result<&'a GraphqlAgentModelProfileOption> {
    let profile = profiles
        .iter()
        .find(|profile| profile.id == model_profile)
        .ok_or_else(|| async_graphql::Error::new("model profile is not available for provider"))?;
    if let Some(reason) = &profile.disabled_reason {
        return Err(async_graphql::Error::new(reason.clone()));
    }
    Ok(profile)
}

fn profile_options(
    profiles: &[(&str, &str)],
    disabled_reason: Option<&str>,
) -> Vec<GraphqlAgentModelProfileOption> {
    profiles
        .iter()
        .map(|(id, label)| GraphqlAgentModelProfileOption {
            id: (*id).to_string(),
            label: (*label).to_string(),
            reasoning_efforts: Vec::new(),
            default_reasoning_effort: None,
            disabled_reason: disabled_reason.map(ToString::to_string),
        })
        .collect()
}

fn metadata_profiles(
    metadata: &Value,
    disabled_reason: Option<&str>,
    supports_reasoning_effort: bool,
) -> Vec<GraphqlAgentModelProfileOption> {
    ProviderModelProfile::from_account_metadata(metadata)
        .into_iter()
        .map(|profile| {
            let reasoning_efforts = if supports_reasoning_effort {
                profile
                    .reasoning_efforts
                    .into_iter()
                    .map(GraphqlReasoningEffort::from)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let default_reasoning_effort = supports_reasoning_effort
                .then_some(profile.default_reasoning_effort)
                .flatten()
                .map(GraphqlReasoningEffort::from)
                .filter(|effort| reasoning_efforts.contains(effort));
            GraphqlAgentModelProfileOption {
                id: profile.id,
                label: profile.label,
                reasoning_efforts,
                default_reasoning_effort,
                disabled_reason: disabled_reason.map(ToString::to_string),
            }
        })
        .collect()
}

pub(super) fn validate_reasoning_effort_for_profile(
    profile: &GraphqlAgentModelProfileOption,
    reasoning_effort: Option<GraphqlReasoningEffort>,
) -> Result<Option<ReasoningEffort>> {
    if profile.reasoning_efforts.is_empty() {
        if reasoning_effort.is_some() {
            return Err(async_graphql::Error::new(
                "reasoning effort is not available for selected model profile",
            ));
        }
        return Ok(None);
    }
    let Some(reasoning_effort) = reasoning_effort else {
        return Err(async_graphql::Error::new(
            "reasoning effort is required for selected model profile",
        ));
    };
    if !profile.reasoning_efforts.contains(&reasoning_effort) {
        return Err(async_graphql::Error::new(
            "reasoning effort is not available for selected model profile",
        ));
    }
    Ok(Some(reasoning_effort.into()))
}
