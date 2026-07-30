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
    pub selection_mode: GraphqlModelPreferenceSelectionMode,
    /// Provider-specific model id or profile id.
    pub model_profile: Option<String>,
    /// Optional explicit reasoning effort for reasoning-capable model profiles.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
}

pub(super) async fn usage_settings(state: &GraphqlState) -> Result<GraphqlUsageSettings> {
    let settings = auxiliary_model_settings(state, AuxiliaryModelTask::ToolProgressAudit).await?;
    Ok(GraphqlUsageSettings {
        progress_audit: GraphqlToolProgressAuditSettings {
            model_preference: settings.preference,
            model_options: settings.options,
        },
    })
}

pub(super) async fn save_tool_progress_audit_preference(
    state: &GraphqlState,
    input: GraphqlSaveToolProgressAuditPreferenceInput,
) -> Result<GraphqlAgentModelPreference> {
    save_auxiliary_model_preference(
        state,
        AuxiliaryModelTask::ToolProgressAudit,
        input.provider_account_id,
        input.selection_mode,
        input.model_profile,
        input.reasoning_effort,
        "graphql_progress_audit_preference",
    )
    .await
}
