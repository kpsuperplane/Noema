use async_graphql::{InputObject, Result, SimpleObject};
use noema_store::AuxiliaryModelTask;

use super::{
    agents::{
        GraphqlAgentModelPreference, GraphqlAgentModelProviderOption,
        GraphqlModelPreferenceSelectionMode, GraphqlReasoningEffort, auxiliary_model_settings,
        save_auxiliary_model_preference,
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
    pub selection_mode: GraphqlModelPreferenceSelectionMode,
    /// Provider-specific model id or profile id.
    pub model_profile: Option<String>,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    pub fast_mode: bool,
}

pub(super) async fn web_fetch_settings(state: &GraphqlState) -> Result<GraphqlWebFetchSettings> {
    let settings = auxiliary_model_settings(state, AuxiliaryModelTask::WebFetchSummarizer).await?;
    Ok(GraphqlWebFetchSettings {
        summarizer: GraphqlWebFetchSummarizerSettings {
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
        input.selection_mode,
        input.model_profile,
        input.reasoning_effort,
        input.fast_mode,
        "graphql_web_fetch_preference",
    )
    .await
}
