use async_graphql::{InputObject, Result, SimpleObject};

use crate::{
    NewAuxiliaryModelPreference, WEB_FETCH_SUMMARIZER_TASK_ID,
    provider::DEFAULT_TOOL_CLASSIFICATION_MODEL,
};

use super::{
    agents::{
        GraphqlAgentModelPreference, GraphqlAgentModelProviderOption, GraphqlReasoningEffort,
        option_from_account, profiles_from_account, provider_disabled_reason,
        refresh_missing_model_profiles, validate_reasoning_effort_for_profile,
    },
    errors::graphql_error,
    schema::GraphqlState,
};

/// Web fetch settings safe to expose in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebFetchSettings")]
pub struct GraphqlWebFetchSettings {
    /// Web fetch summarizer settings.
    pub summarizer: GraphqlWebFetchSummarizerSettings,
}

/// Web fetch summarizer model settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WebFetchSummarizerSettings")]
pub struct GraphqlWebFetchSummarizerSettings {
    /// Built-in default model profile used when no preference is configured.
    pub default_model_profile: String,
    /// Current persisted summarizer model preference, when configured.
    pub model_preference: Option<GraphqlAgentModelPreference>,
    /// Provider/profile options available for the summarizer.
    pub model_options: Vec<GraphqlAgentModelProviderOption>,
}

/// Input for saving the web fetch summarizer preference.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveWebFetchSummarizerPreferenceInput")]
pub struct GraphqlSaveWebFetchSummarizerPreferenceInput {
    /// Provider account id to use.
    pub provider_account_id: String,
    /// Provider-specific model id or profile id.
    pub model_profile: String,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}

pub(super) async fn web_fetch_settings(state: &GraphqlState) -> Result<GraphqlWebFetchSettings> {
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
        .get_auxiliary_model_preference(WEB_FETCH_SUMMARIZER_TASK_ID)
        .await
        .map_err(graphql_error)?;
    Ok(GraphqlWebFetchSettings {
        summarizer: GraphqlWebFetchSummarizerSettings {
            default_model_profile: DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string(),
            model_preference: preference.map(|preference| GraphqlAgentModelPreference {
                provider_kind: preference.provider_kind,
                provider_account_id: preference.provider_account_id,
                model_profile: preference.model_profile,
                reasoning_effort: preference
                    .reasoning_effort
                    .map(GraphqlReasoningEffort::from),
            }),
            model_options: accounts.iter().map(option_from_account).collect(),
        },
    })
}

pub(super) async fn save_web_fetch_summarizer_preference(
    state: &GraphqlState,
    input: GraphqlSaveWebFetchSummarizerPreferenceInput,
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
        .upsert_auxiliary_model_preference(NewAuxiliaryModelPreference {
            task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
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
