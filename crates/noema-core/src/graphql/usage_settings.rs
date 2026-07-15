use async_graphql::{InputObject, Result, SimpleObject};

use crate::{
    NewAuxiliaryModelPreference, provider::DEFAULT_TOOL_CLASSIFICATION_MODEL,
    store::TOOL_PROGRESS_AUDIT_TASK_ID,
};

use super::{
    agents::{
        GraphqlAgentModelPreference, GraphqlAgentModelProviderOption, GraphqlReasoningEffort,
        model_options_from_accounts, provider_disabled_reason, refresh_missing_model_profiles,
        require_selectable_profile, selectable_profiles_from_account,
        validate_reasoning_effort_for_profile,
    },
    errors::graphql_error,
    schema::GraphqlState,
};

/// Safety usage settings safe to expose in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "UsageSettings")]
pub struct GraphqlUsageSettings {
    /// Tool-continuation progress audit model settings.
    pub progress_audit: GraphqlToolProgressAuditSettings,
}

/// Tool-continuation progress audit model settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ToolProgressAuditSettings")]
pub struct GraphqlToolProgressAuditSettings {
    /// Codex/OpenAI default model profile used when no preference is configured.
    pub default_model_profile: String,
    /// Current persisted audit model preference, when configured.
    pub model_preference: Option<GraphqlAgentModelPreference>,
    /// Provider/profile options available for progress audits.
    pub model_options: Vec<GraphqlAgentModelProviderOption>,
}

/// Input for saving the progress audit preference.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveToolProgressAuditPreferenceInput")]
pub struct GraphqlSaveToolProgressAuditPreferenceInput {
    /// Provider account id to use.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}

pub(super) async fn usage_settings(state: &GraphqlState) -> Result<GraphqlUsageSettings> {
    let store = state.store()?;
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
    let preference = store
        .get_auxiliary_model_preference(TOOL_PROGRESS_AUDIT_TASK_ID)
        .await
        .map_err(graphql_error)?;
    let model_options = model_options_from_accounts(store, &accounts).await?;
    Ok(GraphqlUsageSettings {
        progress_audit: GraphqlToolProgressAuditSettings {
            default_model_profile: DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string(),
            model_preference: preference.map(|preference| GraphqlAgentModelPreference {
                provider_kind: preference.provider_kind,
                provider_account_id: preference.provider_account_id,
                model_profile: preference.model_profile,
                reasoning_effort: preference
                    .reasoning_effort
                    .map(GraphqlReasoningEffort::from),
            }),
            model_options,
        },
    })
}

pub(super) async fn save_tool_progress_audit_preference(
    state: &GraphqlState,
    input: GraphqlSaveToolProgressAuditPreferenceInput,
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
    let profiles = selectable_profiles_from_account(store, &account).await?;
    let profile = require_selectable_profile(&profiles, &input.model_profile)?;
    let reasoning_effort = validate_reasoning_effort_for_profile(profile, input.reasoning_effort)?;
    let saved = store
        .upsert_auxiliary_model_preference(NewAuxiliaryModelPreference {
            task_id: TOOL_PROGRESS_AUDIT_TASK_ID.to_string(),
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
