//! Authenticated GraphQL projection for durable governed actions.

use async_graphql::{Enum, InputObject, Json, Result, SimpleObject};
use noema_capabilities::CapabilityDestination;
use noema_store::{
    ExecutionReviewRoute, GovernedActionDecision, GovernedActionRecord, GovernedActionState,
    StoredToolBehavior,
};

use super::runtime_state::GraphqlState;

/// Review route that originated a durable action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "ExecutionReviewRoute")]
pub enum GraphqlExecutionReviewRoute {
    HumanReview,
    LlmReview,
}

/// Complete tool behavior snapshot shown to the human reviewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, SimpleObject)]
#[graphql(name = "ToolBehavior")]
pub struct GraphqlToolBehavior {
    pub read_only: bool,
    pub idempotent: bool,
    pub destructive: bool,
    pub open_world: bool,
}

/// Durable state of one immutable action revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "GovernedActionState")]
pub enum GraphqlGovernedActionState {
    Proposed,
    AwaitingApproval,
    Executable,
    Executing,
    AwaitingAuthentication,
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
    pub review_route: GraphqlExecutionReviewRoute,
    pub behavior: Option<GraphqlToolBehavior>,
    pub safe_summary: String,
    pub destination: Option<Json<serde_json::Value>>,
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
    let action_ids = actions
        .iter()
        .filter(|action| action.capability_name == "web.browse.interact")
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut browser_previews = if action_ids.is_empty() {
        Default::default()
    } else if let Ok(runtime) = state.runtime() {
        runtime
            .browser_action_previews(action_ids, principal.to_string())
            .await
            .unwrap_or_default()
    } else {
        Default::default()
    };
    Ok(actions
        .into_iter()
        .map(|action| {
            let preview = browser_previews.remove(&action.action_id);
            GraphqlGovernedAction::from_record(action, preview)
        })
        .collect())
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
        Self::from_record(action, None)
    }
}

impl GraphqlGovernedAction {
    fn from_record(
        action: GovernedActionRecord,
        ephemeral_browser_preview: Option<serde_json::Value>,
    ) -> Self {
        let display_arguments = ephemeral_browser_preview
            .and_then(|preview| browser_arguments_with_preview(&action.arguments, preview))
            .unwrap_or_else(|| action.arguments.clone());
        let destination = action
            .authorization_context
            .get("destination")
            .cloned()
            .and_then(|value| serde_json::from_value::<CapabilityDestination>(value).ok())
            .and_then(|value| serde_json::to_value(value).ok())
            .map(Json);
        Self {
            action_id: action.action_id,
            revision: action.revision,
            conversation_id: action.conversation_id,
            task_id: action.task_id,
            run_id: action.run_id,
            capability_name: action.capability_name,
            review_route: action.review_route.into(),
            behavior: action.behavior.map(Into::into),
            safe_summary: action.safe_summary,
            destination,
            arguments: Json(display_arguments),
            state: action.state.into(),
            output: action.output.map(Json),
            failure_code: action.failure_code,
        }
    }
}

fn browser_arguments_with_preview(
    arguments: &serde_json::Value,
    preview: serde_json::Value,
) -> Option<serde_json::Value> {
    let mut arguments = arguments.as_object()?.clone();
    arguments.extend(preview.as_object()?.clone());
    Some(serde_json::Value::Object(arguments))
}

graphql_enum_from!(ExecutionReviewRoute => GraphqlExecutionReviewRoute {
    HumanReview => HumanReview,
    LlmReview => LlmReview,
});

impl From<StoredToolBehavior> for GraphqlToolBehavior {
    fn from(behavior: StoredToolBehavior) -> Self {
        Self {
            read_only: behavior.read_only,
            idempotent: behavior.idempotent,
            destructive: behavior.destructive,
            open_world: behavior.open_world,
        }
    }
}

graphql_enum_from!(GovernedActionState => GraphqlGovernedActionState {
    Proposed => Proposed,
    AwaitingApproval => AwaitingApproval,
    Executable => Executable,
    Executing => Executing,
    AwaitingAuthentication => AwaitingAuthentication,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_arguments_show_the_exact_reviewed_values() {
        let action = GovernedActionRecord {
            action_id: "action:test".to_string(),
            revision: 1,
            owner_human_id: "human:local".to_string(),
            conversation_id: None,
            turn_id: None,
            task_id: None,
            run_id: None,
            requesting_agent_id: "agent:primary".to_string(),
            capability_name: "web.browse.interact".to_string(),
            operation_token: "opaque".to_string(),
            review_route: ExecutionReviewRoute::HumanReview,
            behavior: Some(StoredToolBehavior {
                read_only: false,
                idempotent: false,
                destructive: false,
                open_world: true,
            }),
            arguments: serde_json::json!({
                "snapshot_revision": 7,
                "ref": "e2",
                "action": "fill",
                "value": "secret-marker",
            }),
            arguments_sha256: "a".repeat(64),
            input_schema: serde_json::json!({"type": "object"}),
            authorization_context: serde_json::json!({
                "origin": "test",
                "destination": {
                    "service_id": "adapter",
                    "connection_id": "connection:test",
                    "account_id": "account:test",
                    "revision": "revision:1"
                }
            }),
            safe_summary: "fixture write".to_string(),
            state: GovernedActionState::AwaitingApproval,
            output: None,
            failure_code: None,
        };
        let projection = GraphqlGovernedAction::from_record(
            action,
            Some(serde_json::json!({
                "kind": "browser_interaction",
                "page": {"url": "https://example.com/form", "title": "Example form"},
                "target": {"ref": "e2", "role": "textbox", "name": "Name"},
            })),
        );
        let encoded = serde_json::to_string(&projection.arguments.0).expect("arguments");
        assert!(encoded.contains("secret-marker"));
        assert_eq!(projection.arguments.0["value"], "secret-marker");
        assert_eq!(projection.arguments.0["target"]["name"], "Name");
        assert_eq!(
            projection.destination.as_ref().expect("destination").0["connection_id"],
            "connection:test"
        );
    }
}
