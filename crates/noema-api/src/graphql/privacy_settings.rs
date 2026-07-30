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

/// Privacy settings safe to expose in Settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "PrivacySettings")]
pub struct GraphqlPrivacySettings {
    /// Model used to review governed writes and exports.
    pub reviewer: GraphqlActionReviewerSettings,
}

/// Governed-action reviewer model settings.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ActionReviewerSettings")]
pub struct GraphqlActionReviewerSettings {
    /// Explicit reviewer preference, or none when action review requires human approval.
    pub model_preference: Option<GraphqlAgentModelPreference>,
    /// Provider/profile options available for action review.
    pub model_options: Vec<GraphqlAgentModelProviderOption>,
}

/// Input for saving the governed-action reviewer preference.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "SaveActionReviewerPreferenceInput")]
pub struct GraphqlSaveActionReviewerPreferenceInput {
    /// Provider account id to use.
    pub provider_account_id: String,
    pub selection_mode: GraphqlModelPreferenceSelectionMode,
    /// Provider-specific model id or profile id.
    pub model_profile: Option<String>,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}

pub(super) async fn privacy_settings(state: &GraphqlState) -> Result<GraphqlPrivacySettings> {
    let settings = auxiliary_model_settings(state, AuxiliaryModelTask::ActionReviewer).await?;
    Ok(GraphqlPrivacySettings {
        reviewer: GraphqlActionReviewerSettings {
            model_preference: settings.preference,
            model_options: settings.options,
        },
    })
}

pub(super) async fn save_action_reviewer_preference(
    state: &GraphqlState,
    input: GraphqlSaveActionReviewerPreferenceInput,
) -> Result<GraphqlAgentModelPreference> {
    save_auxiliary_model_preference(
        state,
        AuxiliaryModelTask::ActionReviewer,
        input.provider_account_id,
        input.selection_mode,
        input.model_profile,
        input.reasoning_effort,
        "graphql_action_reviewer_preference",
    )
    .await
}
