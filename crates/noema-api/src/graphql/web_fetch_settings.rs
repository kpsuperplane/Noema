use async_graphql::{InputObject, Result, SimpleObject};
use noema_providers::DEFAULT_TOOL_CLASSIFICATION_MODEL;

use noema_store::{NewAuxiliaryModelPreference, WEB_FETCH_SUMMARIZER_TASK_ID};

use super::{
    agents::{
        GraphqlAgentModelPreference, GraphqlAgentModelProviderOption, GraphqlReasoningEffort,
        active_default_model_accounts, model_options_from_accounts, provider_disabled_reason,
        require_selectable_profile, selectable_model_account, selectable_profiles_from_account,
        validate_reasoning_effort_for_profile,
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
    let accounts = active_default_model_accounts(state).await?;
    let preference = store
        .get_auxiliary_model_preference(WEB_FETCH_SUMMARIZER_TASK_ID)
        .await
        .map_err(graphql_error)?;
    let model_options = model_options_from_accounts(store, &accounts).await?;
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
            model_options,
        },
    })
}

pub(super) async fn save_web_fetch_summarizer_preference(
    state: &GraphqlState,
    input: GraphqlSaveWebFetchSummarizerPreferenceInput,
) -> Result<GraphqlAgentModelPreference> {
    let store = state.store()?;
    let account = selectable_model_account(state, &input.provider_account_id).await?;
    if let Some(reason) = provider_disabled_reason(&account) {
        return Err(async_graphql::Error::new(reason));
    }
    let profiles = selectable_profiles_from_account(store, &account).await?;
    let profile = require_selectable_profile(&profiles, &input.model_profile)?;
    let reasoning_effort = validate_reasoning_effort_for_profile(profile, input.reasoning_effort)?;
    let ready_selection = super::provider_selection::prove_ready_selection(
        state,
        &account.provider_kind,
        &account.provider_account_id,
        &input.model_profile,
        reasoning_effort,
        "graphql_web_fetch_preference",
    )
    .await?;
    let preference = NewAuxiliaryModelPreference {
        task_id: WEB_FETCH_SUMMARIZER_TASK_ID.to_string(),
        provider_kind: account.provider_kind,
        provider_account_id: account.provider_account_id,
        model_profile: input.model_profile,
        reasoning_effort,
    };
    let saved = store
        .upsert_auxiliary_model_preference_with_ready_selection(preference, &ready_selection)
        .await
        .map_err(graphql_error)?;
    Ok(GraphqlAgentModelPreference {
        provider_kind: saved.provider_kind,
        provider_account_id: saved.provider_account_id,
        model_profile: saved.model_profile,
        reasoning_effort: saved.reasoning_effort.map(GraphqlReasoningEffort::from),
    })
}
