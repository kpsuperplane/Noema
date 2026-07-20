use async_graphql::{InputObject, Result, SimpleObject};
use noema_providers::DEFAULT_TOOL_CLASSIFICATION_MODEL;

use noema_store::AuxiliaryModelTask;

use super::{
    agents::{
        GraphqlAgentModelPreference, GraphqlAgentModelProviderOption, GraphqlReasoningEffort,
        auxiliary_model_settings, save_auxiliary_model_preference,
    },
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
    let settings = auxiliary_model_settings(state, AuxiliaryModelTask::WebFetchSummarizer).await?;
    Ok(GraphqlWebFetchSettings {
        summarizer: GraphqlWebFetchSummarizerSettings {
            default_model_profile: DEFAULT_TOOL_CLASSIFICATION_MODEL.to_string(),
            model_preference: settings.preference,
            model_options: settings.options,
        },
    })
}

pub(super) async fn save_web_fetch_summarizer_preference(
    state: &GraphqlState,
    input: GraphqlSaveWebFetchSummarizerPreferenceInput,
) -> Result<GraphqlAgentModelPreference> {
    save_auxiliary_model_preference(
        state,
        AuxiliaryModelTask::WebFetchSummarizer,
        input.provider_account_id,
        input.model_profile,
        input.reasoning_effort,
        "graphql_web_fetch_preference",
    )
    .await
}
