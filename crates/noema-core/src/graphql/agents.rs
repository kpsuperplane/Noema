use async_graphql::{InputObject, Result, SimpleObject};
use serde_json::Value;

use crate::{
    AgentRecord, AgentRuntimePreferenceRecord, NewAgentRuntimePreference, ProviderAccountRecord,
    ProviderAccountStatus, provider::model_catalog::refresh_provider_model_profiles,
};

use super::{errors::graphql_error, schema::GraphqlState};

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
}

/// One selectable model/profile.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "AgentModelProfileOption")]
pub struct GraphqlAgentModelProfileOption {
    /// Stable profile or model id.
    pub id: String,
    /// User-facing label.
    pub label: String,
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

async fn refresh_missing_model_profiles(
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
    if !profiles
        .iter()
        .any(|profile| profile.id == input.model_profile)
    {
        return Err(async_graphql::Error::new(
            "model profile is not available for provider",
        ));
    }
    let saved = store
        .upsert_agent_runtime_preference(NewAgentRuntimePreference {
            agent_id: input.agent_id,
            provider_kind: account.provider_kind,
            provider_account_id: account.provider_account_id,
            model_profile: input.model_profile,
        })
        .await
        .map_err(graphql_error)?;
    Ok(GraphqlAgentModelPreference {
        provider_kind: saved.provider_kind,
        provider_account_id: saved.provider_account_id,
        model_profile: saved.model_profile,
    })
}

fn option_from_account(account: &ProviderAccountRecord) -> GraphqlAgentModelProviderOption {
    let disabled_reason = provider_disabled_reason(account);
    GraphqlAgentModelProviderOption {
        provider_kind: account.provider_kind.clone(),
        provider_account_id: account.provider_account_id.clone(),
        provider_display_name: account.display_name.clone(),
        status: account.status.into(),
        profiles: profiles_from_account(account, disabled_reason.as_deref()),
        disabled_reason,
    }
}

fn provider_disabled_reason(account: &ProviderAccountRecord) -> Option<String> {
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
            Some(GraphqlAgentModelProfileOption {
                id: id.to_string(),
                label: label.to_string(),
                disabled_reason: disabled_reason.map(ToString::to_string),
            })
        })
        .collect()
}
