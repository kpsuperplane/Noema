use async_graphql::{Enum, InputObject, Result, SimpleObject};
use serde_json::Value;

use crate::{
    AgentRecord, AgentRuntimePreferenceRecord, NewAgentRuntimePreference, ProviderAccountRecord,
    ProviderAccountStatus,
    config::DEFAULT_FOUNDATION_LOCAL_PROFILE,
    provider::{
        DEFAULT_TOOL_CLASSIFICATION_MODEL, ReasoningEffort,
        model_catalog::refresh_provider_model_profiles,
    },
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

impl From<ReasoningEffort> for GraphqlReasoningEffort {
    fn from(value: ReasoningEffort) -> Self {
        match value {
            ReasoningEffort::None => Self::None,
            ReasoningEffort::Minimal => Self::Minimal,
            ReasoningEffort::Low => Self::Low,
            ReasoningEffort::Medium => Self::Medium,
            ReasoningEffort::High => Self::High,
            ReasoningEffort::XHigh => Self::Xhigh,
        }
    }
}

impl From<GraphqlReasoningEffort> for ReasoningEffort {
    fn from(value: GraphqlReasoningEffort) -> Self {
        match value {
            GraphqlReasoningEffort::None => Self::None,
            GraphqlReasoningEffort::Minimal => Self::Minimal,
            GraphqlReasoningEffort::Low => Self::Low,
            GraphqlReasoningEffort::Medium => Self::Medium,
            GraphqlReasoningEffort::High => Self::High,
            GraphqlReasoningEffort::Xhigh => Self::XHigh,
        }
    }
}

/// Agent model preference safe to expose in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AgentModelPreference")]
pub struct GraphqlAgentModelPreference {
    /// Provider kind selected for this agent.
    pub provider_kind: String,
    /// Provider account id selected for this agent.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
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
    /// Provider-specific default profile for auxiliary model preferences.
    pub default_model_profile: Option<String>,
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
    pub model_profile: String,
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
        accounts: &[ProviderAccountRecord],
    ) -> Self {
        let is_primary = agent.agent_id == "agent:primary";
        Self {
            agent_id: agent.agent_id,
            display_name: agent.display_name,
            is_primary,
            model_preference: preference.map(|preference| GraphqlAgentModelPreference {
                provider_kind: preference.provider_kind,
                provider_account_id: preference.provider_account_id,
                model_profile: preference.model_profile,
                reasoning_effort: preference
                    .reasoning_effort
                    .map(GraphqlReasoningEffort::from),
            }),
            model_options: accounts.iter().map(option_from_account).collect(),
        }
    }
}

pub(super) async fn agents(state: &GraphqlState) -> Result<Vec<GraphqlAgent>> {
    let store = state.store()?;
    let agents = store.list_agents().await.map_err(graphql_error)?;
    super::provider_accounts::refresh_foundation_local_availability(state).await;
    let mut accounts = store
        .active_default_provider_accounts()
        .await
        .map_err(graphql_error)?;
    refresh_missing_model_profiles(state, store, &accounts).await;
    accounts = store
        .active_default_provider_accounts()
        .await
        .map_err(graphql_error)?;
    let mut output = Vec::with_capacity(agents.len());
    for agent in agents {
        let preference = store
            .get_agent_runtime_preference(&agent.agent_id)
            .await
            .map_err(graphql_error)?;
        output.push(GraphqlAgent::from_parts(agent, preference, &accounts));
    }
    Ok(output)
}

pub(super) async fn refresh_missing_model_profiles(
    state: &GraphqlState,
    store: &crate::NoemaStore,
    accounts: &[ProviderAccountRecord],
) {
    let Ok(paths) = state.paths() else {
        return;
    };
    for account in accounts {
        let _ = refresh_provider_model_profiles(store, paths, account).await;
    }
}

pub(super) async fn save_agent_model_preference(
    state: &GraphqlState,
    input: GraphqlSaveAgentModelPreferenceInput,
) -> Result<GraphqlAgentModelPreference> {
    let store = state.store()?;
    let account = store
        .get_provider_account(&input.provider_account_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("provider account not found"))?;
    if !account.is_active || !account.is_default {
        return Err(async_graphql::Error::new(
            "provider account is not selectable",
        ));
    }
    if let Some(reason) = provider_disabled_reason(&account) {
        return Err(async_graphql::Error::new(reason));
    }
    let profiles = profiles_from_account(&account, None);
    let Some(profile) = profiles
        .iter()
        .find(|profile| profile.id == input.model_profile)
    else {
        return Err(async_graphql::Error::new(
            "model profile is not available for provider",
        ));
    };
    let reasoning_effort = validate_reasoning_effort_for_profile(profile, input.reasoning_effort)?;
    let saved = store
        .upsert_agent_runtime_preference(NewAgentRuntimePreference {
            agent_id: input.agent_id,
            provider_kind: account.provider_kind,
            provider_account_id: account.provider_account_id,
            model_profile: input.model_profile,
            reasoning_effort,
        })
        .await
        .map_err(graphql_error)?;
    Ok(GraphqlAgentModelPreference {
        provider_kind: saved.provider_kind,
        provider_account_id: saved.provider_account_id,
        model_profile: saved.model_profile,
        reasoning_effort: saved.reasoning_effort.map(GraphqlReasoningEffort::from),
    })
}

pub(super) fn option_from_account(
    account: &ProviderAccountRecord,
) -> GraphqlAgentModelProviderOption {
    let disabled_reason = provider_disabled_reason(account);
    let profiles = profiles_from_account(account, disabled_reason.as_deref());
    let default_model_profile = default_model_profile_for_provider(account, &profiles);
    GraphqlAgentModelProviderOption {
        provider_kind: account.provider_kind.clone(),
        provider_account_id: account.provider_account_id.clone(),
        provider_display_name: account.display_name.clone(),
        status: account.status.into(),
        profiles,
        default_model_profile,
        disabled_reason,
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

fn default_model_profile_for_provider(
    account: &ProviderAccountRecord,
    profiles: &[GraphqlAgentModelProfileOption],
) -> Option<String> {
    let first_profile = || profiles.first().map(|profile| profile.id.clone());
    match account.provider_kind.as_str() {
        "codex" | "openai" => profiles
            .iter()
            .find(|profile| profile.id == DEFAULT_TOOL_CLASSIFICATION_MODEL)
            .map(|profile| profile.id.clone())
            .or_else(first_profile),
        "foundation_local" => profiles
            .iter()
            .find(|profile| profile.id == DEFAULT_FOUNDATION_LOCAL_PROFILE)
            .map(|profile| profile.id.clone())
            .or_else(first_profile),
        _ => first_profile(),
    }
}

pub(super) fn profiles_from_account(
    account: &ProviderAccountRecord,
    disabled_reason: Option<&str>,
) -> Vec<GraphqlAgentModelProfileOption> {
    let metadata_profiles = metadata_profiles(&account.metadata, disabled_reason);
    if !metadata_profiles.is_empty() {
        return metadata_profiles;
    }
    match account.provider_kind.as_str() {
        "foundation_local" => profile_options(&[("default", "Default on-device")], disabled_reason),
        _ => Vec::new(),
    }
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
) -> Vec<GraphqlAgentModelProfileOption> {
    metadata
        .get("profiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|profile| {
            let id = profile.get("id")?.as_str()?.trim();
            if id.is_empty() {
                return None;
            }
            let label = profile
                .get("label")
                .and_then(Value::as_str)
                .filter(|label| !label.trim().is_empty())
                .unwrap_or(id);
            let reasoning_efforts = profile
                .get("reasoning_efforts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(reasoning_effort_from_metadata)
                .map(GraphqlReasoningEffort::from)
                .collect::<Vec<_>>();
            let default_reasoning_effort = profile
                .get("default_reasoning_effort")
                .and_then(Value::as_str)
                .and_then(reasoning_effort_from_metadata)
                .map(GraphqlReasoningEffort::from)
                .filter(|effort| reasoning_efforts.contains(effort));
            Some(GraphqlAgentModelProfileOption {
                id: id.to_string(),
                label: label.to_string(),
                reasoning_efforts,
                default_reasoning_effort,
                disabled_reason: disabled_reason.map(ToString::to_string),
            })
        })
        .collect()
}

fn reasoning_effort_from_metadata(value: &str) -> Option<ReasoningEffort> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Some(ReasoningEffort::None),
        "minimal" => Some(ReasoningEffort::Minimal),
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" => Some(ReasoningEffort::XHigh),
        _ => None,
    }
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
