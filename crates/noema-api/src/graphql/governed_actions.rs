//! Authenticated GraphQL projection for durable governed actions.

use async_graphql::{Enum, InputObject, Json, Result, SimpleObject};
use noema_capabilities::CapabilityDestination;
use noema_store::{
    ExecutionReviewRoute, GovernedActionAssessmentRecord, GovernedActionDecision,
    GovernedActionRecord, GovernedActionState, GovernedAssessmentStatus, GovernedAuthorization,
    GovernedRisk, StoredToolBehavior,
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

/// Whether the model reviewer produced a valid assessment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "GovernedAssessmentStatus")]
pub enum GraphqlGovernedAssessmentStatus {
    Completed,
    ReviewerUnavailable,
    InvalidResponse,
}

/// How directly authenticated human authority covered the action.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "GovernedAuthorization")]
pub enum GraphqlGovernedAuthorization {
    Explicit,
    Substantive,
    Weak,
    Absent,
}

/// Consequence if the proposed action is wrong.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "GovernedRisk")]
pub enum GraphqlGovernedRisk {
    Low,
    Medium,
    High,
    Critical,
}

/// Model review that determined whether this action requires approval.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "GovernedActionAssessment")]
pub struct GraphqlGovernedActionAssessment {
    pub status: GraphqlGovernedAssessmentStatus,
    pub authorization: Option<GraphqlGovernedAuthorization>,
    pub risk: Option<GraphqlGovernedRisk>,
    pub reason_codes: Vec<String>,
    pub explanation: String,
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
    pub assessment: Option<GraphqlGovernedActionAssessment>,
    pub browser_session_available: Option<bool>,
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
    let browser_actions = actions
        .iter()
        .filter(|action| is_session_bound_browser_action(&action.capability_name))
        .map(|action| (action.action_id.clone(), action.revision))
        .collect::<Vec<_>>();
    let mut browser_availability = if browser_actions.is_empty() {
        Default::default()
    } else if let Ok(runtime) = state.runtime() {
        runtime
            .browser_action_session_availability(browser_actions, principal.to_string())
            .await
            .unwrap_or_default()
    } else {
        Default::default()
    };
    Ok(actions
        .into_iter()
        .map(|action| {
            let session_available = browser_availability.remove(&action.action_id);
            GraphqlGovernedAction::from_record(action, session_available)
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
    fn from_record(action: GovernedActionRecord, browser_session_available: Option<bool>) -> Self {
        let display_arguments = action
            .authorization_context
            .get("browser_review_context")
            .cloned()
            .and_then(|context| browser_arguments_with_context(&action.arguments, context))
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
            assessment: action.assessment.map(Into::into),
            browser_session_available,
            state: action.state.into(),
            output: action.output.map(Json),
            failure_code: action.failure_code,
        }
    }
}

fn browser_arguments_with_context(
    arguments: &serde_json::Value,
    context: serde_json::Value,
) -> Option<serde_json::Value> {
    let mut arguments = arguments.as_object()?.clone();
    arguments.extend(context.as_object()?.clone());
    Some(serde_json::Value::Object(arguments))
}

fn is_session_bound_browser_action(capability_name: &str) -> bool {
    matches!(
        capability_name,
        "web.browse.navigate" | "web.browse.interact" | "web.browse.history"
    )
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

impl From<GovernedActionAssessmentRecord> for GraphqlGovernedActionAssessment {
    fn from(assessment: GovernedActionAssessmentRecord) -> Self {
        Self {
            status: assessment.status.into(),
            authorization: assessment.authorization.map(Into::into),
            risk: assessment.risk.map(Into::into),
            reason_codes: assessment.reason_codes,
            explanation: assessment.explanation,
        }
    }
}

graphql_enum_from!(GovernedAssessmentStatus => GraphqlGovernedAssessmentStatus {
    Completed => Completed,
    ReviewerUnavailable => ReviewerUnavailable,
    InvalidResponse => InvalidResponse,
});

graphql_enum_from!(GovernedAuthorization => GraphqlGovernedAuthorization {
    Explicit => Explicit,
    Substantive => Substantive,
    Weak => Weak,
    Absent => Absent,
});

graphql_enum_from!(GovernedRisk => GraphqlGovernedRisk {
    Low => Low,
    Medium => Medium,
    High => High,
    Critical => Critical,
});

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
            approval_item_id: None,
            task_id: None,
            run_id: None,
            requesting_agent_id: "agent:primary".to_string(),
            capability_name: "web.browse.interact".to_string(),
            operation_token: "opaque".to_string(),
            review_route: ExecutionReviewRoute::LlmReview,
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
                },
                "browser_review_context": {
                    "kind": "browser_interaction",
                    "page": {"url": "https://example.com/form", "title": "Example form"},
                    "target": {"ref": "e2", "role": "textbox", "name": "Name"}
                },
            }),
            safe_summary: "fixture write".to_string(),
            state: GovernedActionState::AwaitingApproval,
            output: None,
            failure_code: None,
            assessment: Some(GovernedActionAssessmentRecord {
                status: GovernedAssessmentStatus::Completed,
                reviewer_selection: None,
                authorization: Some(GovernedAuthorization::Substantive),
                risk: Some(GovernedRisk::High),
                reason_codes: vec!["sensitive_data".to_string()],
                explanation: "The action may disclose private data.".to_string(),
            }),
        };
        let projection = GraphqlGovernedAction::from_record(action, Some(true));
        let encoded = serde_json::to_string(&projection.arguments.0).expect("arguments");
        assert!(encoded.contains("secret-marker"));
        assert_eq!(projection.arguments.0["value"], "secret-marker");
        assert_eq!(projection.arguments.0["target"]["name"], "Name");
        assert_eq!(projection.browser_session_available, Some(true));
        let assessment = projection.assessment.expect("assessment");
        assert_eq!(assessment.risk, Some(GraphqlGovernedRisk::High));
        assert_eq!(assessment.reason_codes, ["sensitive_data"]);
        assert_eq!(
            projection.destination.as_ref().expect("destination").0["connection_id"],
            "connection:test"
        );
    }
}
