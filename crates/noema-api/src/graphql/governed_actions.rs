//! Authenticated GraphQL projection for durable governed actions.

use async_graphql::{Enum, InputObject, Json, Result, SimpleObject};
use noema_store::{
    GovernedActionDecision, GovernedActionEffect, GovernedActionRecord, GovernedActionState,
};

use super::runtime_state::GraphqlState;

/// External side-effect class shown to the human reviewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "GovernedActionEffect")]
pub enum GraphqlGovernedActionEffect {
    Write,
    Export,
    WriteAndExport,
}

/// Durable state of one immutable action revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "GovernedActionState")]
pub enum GraphqlGovernedActionState {
    Proposed,
    AwaitingApproval,
    Executable,
    Executing,
    Succeeded,
    Failed,
    OutcomeUncertain,
    Declined,
    Superseded,
    Cancelled,
}

/// One human decision, applied with an expected revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "GovernedActionDecision")]
pub enum GraphqlGovernedActionDecision {
    Approve,
    Decline,
}

/// Input for resolving one exact action revision.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "ResolveGovernedActionInput")]
pub struct GraphqlResolveGovernedActionInput {
    pub action_id: String,
    pub expected_revision: u64,
    pub decision: GraphqlGovernedActionDecision,
}

/// Human-readable projection of one governed external action.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "GovernedAction")]
pub struct GraphqlGovernedAction {
    pub action_id: String,
    pub revision: u64,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub capability_name: String,
    pub effect: GraphqlGovernedActionEffect,
    pub safe_summary: String,
    pub arguments: Json<serde_json::Value>,
    pub state: GraphqlGovernedActionState,
    pub output: Option<Json<serde_json::Value>>,
    pub failure_code: Option<String>,
}

pub(super) async fn pending_governed_actions(
    state: &GraphqlState,
    principal: &str,
    conversation_id: Option<String>,
    task_id: Option<String>,
    first: Option<i32>,
) -> Result<Vec<GraphqlGovernedAction>> {
    let first = usize::try_from(first.unwrap_or(50).clamp(1, 100)).unwrap_or(50);
    let actions = state
        .store()?
        .list_pending_governed_actions(
            principal,
            conversation_id.as_deref(),
            task_id.as_deref(),
            first,
        )
        .await?;
    Ok(actions.into_iter().map(Into::into).collect())
}

pub(super) async fn resolve_governed_action(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlResolveGovernedActionInput,
) -> Result<GraphqlGovernedAction> {
    let action = state
        .runtime()?
        .resolve_governed_action(
            input.action_id,
            input.expected_revision,
            principal.to_string(),
            input.decision.into(),
        )
        .await?;
    Ok(action.into())
}

impl From<GovernedActionRecord> for GraphqlGovernedAction {
    fn from(action: GovernedActionRecord) -> Self {
        Self {
            action_id: action.action_id,
            revision: action.revision,
            conversation_id: action.conversation_id,
            task_id: action.task_id,
            run_id: action.run_id,
            capability_name: action.capability_name,
            effect: action.effect.into(),
            safe_summary: action.safe_summary,
            arguments: Json(action.arguments),
            state: action.state.into(),
            output: action.output.map(Json),
            failure_code: action.failure_code,
        }
    }
}

graphql_enum_from!(GovernedActionEffect => GraphqlGovernedActionEffect {
    Write => Write,
    Export => Export,
    WriteAndExport => WriteAndExport,
});

graphql_enum_from!(GovernedActionState => GraphqlGovernedActionState {
    Proposed => Proposed,
    AwaitingApproval => AwaitingApproval,
    Executable => Executable,
    Executing => Executing,
    Succeeded => Succeeded,
    Failed => Failed,
    OutcomeUncertain => OutcomeUncertain,
    Declined => Declined,
    Superseded => Superseded,
    Cancelled => Cancelled,
});

graphql_enum_from!(GraphqlGovernedActionDecision => GovernedActionDecision {
    Approve => Approve,
    Decline => Decline,
});
