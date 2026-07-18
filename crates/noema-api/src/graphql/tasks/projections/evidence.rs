use async_graphql::{Json, SimpleObject};
use noema_tasks::{
    AgentRunItemRecord, AgentRunRecord, TaskGateRecord, TaskReviewRecord, TaskSubmissionRecord,
};

use crate::graphql::tasks::GraphqlApprovalDecision;

use super::*;

/// Task gate category.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskGate")]
pub struct GraphqlTaskGate {
    /// Gate identity.
    pub gate_id: String,
    /// Task generation.
    pub task_generation: i64,
    /// Gate kind.
    pub kind: GraphqlTaskGateKind,
    /// Gate state.
    pub state: GraphqlTaskGateState,
    /// Recovery reason, when any.
    pub recovery_reason: Option<GraphqlTaskRecoveryReason>,
    /// Explicit recovery continuation role, when any.
    pub retry_run_kind: Option<GraphqlTaskRunKind>,
    /// Human prompt.
    pub prompt: String,
    /// Bounded context.
    pub context_markdown: String,
    /// Opener actor.
    pub opened_by: String,
    /// Opening run, when any.
    pub originating_run_id: Option<String>,
    /// Open timestamp.
    pub opened_at: String,
    /// Resolver actor, when resolved.
    pub resolved_by: Option<String>,
    /// Resolution timestamp, when resolved.
    pub resolved_at: Option<String>,
    /// Resolution message identity, when resolved.
    pub resolution: Option<String>,
}

impl TryFrom<TaskGateRecord> for GraphqlTaskGate {
    type Error = async_graphql::Error;

    fn try_from(value: TaskGateRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            gate_id: value.gate_id.into_string(),
            task_generation: exact_u64(value.task_generation)?,
            kind: value.kind.into(),
            state: value.state.into(),
            recovery_reason: value.recovery_reason.map(Into::into),
            retry_run_kind: value.retry_run_kind.map(Into::into),
            prompt: value.prompt_markdown,
            context_markdown: value.context_markdown,
            opened_by: value.opened_by_actor_id,
            originating_run_id: value.originating_run_id,
            opened_at: value.opened_at,
            resolved_by: value.resolved_by_actor_id,
            resolved_at: value.resolved_at,
            resolution: value.resolution_message_id.map(|id| id.into_string()),
        })
    }
}

/// Human message in the immutable task history.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskMessage")]
pub struct GraphqlTaskMessage {
    /// Message identity.
    pub message_id: String,
    /// Task generation.
    pub task_generation: i64,
    /// Message kind.
    pub kind: GraphqlTaskMessageKind,
    /// Safe Markdown body.
    pub body_markdown: String,
    /// Author actor.
    pub author: String,
    /// Associated gate, when any.
    pub gate_id: Option<String>,
    /// Associated contract, when any.
    pub contract_id: Option<String>,
    /// Structured approval decision, when any.
    pub approval_decision: Option<GraphqlApprovalDecision>,
    /// Consumer run, when consumed.
    pub consumed_by_run_id: Option<String>,
    /// Consumption timestamp, when consumed.
    pub consumed_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
}

/// Current run projection, intentionally separate from task stage.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "CurrentRunSummary")]
pub struct GraphqlCurrentRunSummary {
    /// Run identity.
    pub run_id: String,
    /// Planner, Executor, or Reviewer.
    pub kind: GraphqlTaskRunKind,
    /// Run-local queue/lease status.
    pub status: GraphqlTaskRunStatus,
    /// Lineage attempt.
    pub attempt_index: i64,
    /// Contract identity, absent for Planner.
    pub contract_id: Option<String>,
    /// Queue timestamp.
    pub queued_at: String,
    /// Start timestamp.
    pub started_at: Option<String>,
    /// Last update timestamp.
    pub updated_at: String,
    /// Safe activity label.
    pub activity_label: String,
}

impl From<AgentRunRecord> for GraphqlCurrentRunSummary {
    fn from(value: AgentRunRecord) -> Self {
        let activity_label = format!("{} {}", value.run_kind.as_str(), value.status.as_str());
        Self {
            run_id: value.run_id,
            kind: value.run_kind.into(),
            status: value.status.into(),
            attempt_index: i64::from(value.attempt_index),
            contract_id: value.contract_id.map(|id| id.into_string()),
            queued_at: value.queued_at,
            started_at: value.started_at,
            updated_at: value.updated_at,
            activity_label,
        }
    }
}

/// Immutable submission evidence.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskSubmission")]
pub struct GraphqlTaskSubmission {
    /// Submission identity.
    pub submission_id: String,
    /// Contract evaluated.
    pub contract_id: String,
    /// Executor run.
    pub executor_run_id: String,
    /// Review round.
    pub review_round: i64,
    /// Short summary.
    pub summary: String,
    /// Complete result Markdown.
    pub result_markdown: String,
    /// Criterion evidence.
    pub criteria: Vec<GraphqlTaskSubmissionCriterion>,
    /// Linked immutable artifact versions.
    pub artifacts: Vec<GraphqlTaskSubmissionArtifact>,
    /// Creation timestamp.
    pub created_at: String,
}

/// Submission criterion evidence.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskSubmissionCriterion")]
pub struct GraphqlTaskSubmissionCriterion {
    /// Criterion identity.
    pub criterion_id: String,
    /// Evidence Markdown.
    pub evidence_markdown: String,
}

/// Immutable artifact link shown with a submission.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskSubmissionArtifact")]
pub struct GraphqlTaskSubmissionArtifact {
    /// Artifact identity.
    pub artifact_id: String,
    /// Artifact version identity.
    pub artifact_version_id: String,
    /// Artifact title.
    pub title: String,
    /// Artifact kind.
    pub artifact_kind: String,
    /// Storage kind.
    pub storage_kind: crate::graphql::artifacts::GraphqlArtifactStorageKind,
    /// Media type, when present.
    pub media_type: Option<String>,
    /// Local download route, when present.
    pub download_url: Option<String>,
    /// External URL, when present.
    pub external_url: Option<String>,
}

impl From<TaskSubmissionRecord> for GraphqlTaskSubmission {
    fn from(value: TaskSubmissionRecord) -> Self {
        Self {
            submission_id: value.submission_id,
            contract_id: value.contract_id.into_string(),
            executor_run_id: value.executor_run_id,
            review_round: i64::from(value.review_round),
            summary: value.summary,
            result_markdown: value.result_markdown,
            criteria: value
                .criteria
                .into_iter()
                .map(|criterion| GraphqlTaskSubmissionCriterion {
                    criterion_id: criterion.criterion_id,
                    evidence_markdown: criterion.evidence_markdown,
                })
                .collect(),
            artifacts: value
                .artifacts
                .into_iter()
                .map(|linked| {
                    let (download_url, external_url) = match linked.version.storage {
                        noema_artifacts::ArtifactVersionStorage::LocalFile { .. } => (
                            Some(noema_artifacts::artifact_download_url(
                                &linked.version.artifact_version_id,
                            )),
                            None,
                        ),
                        noema_artifacts::ArtifactVersionStorage::ExternalUrl { url } => {
                            (None, Some(url))
                        }
                    };
                    GraphqlTaskSubmissionArtifact {
                        artifact_id: linked.artifact.artifact_id,
                        artifact_version_id: linked.version.artifact_version_id,
                        title: linked.artifact.title,
                        artifact_kind: linked.artifact.artifact_kind,
                        storage_kind: linked.artifact.storage_kind.into(),
                        media_type: linked.version.media_type,
                        download_url,
                        external_url,
                    }
                })
                .collect(),
            created_at: value.created_at,
        }
    }
}

/// Immutable reviewer criterion outcome.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskReviewCriterion")]
pub struct GraphqlTaskReviewCriterion {
    /// Criterion identity.
    pub criterion_id: String,
    /// Pass, fail, or uncertain.
    pub outcome: GraphqlTaskCriterionOutcome,
    /// Evidence Markdown.
    pub evidence_markdown: Option<String>,
    /// Reviewer feedback.
    pub feedback: Option<String>,
}

/// Immutable reviewer decision.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskReview")]
pub struct GraphqlTaskReview {
    /// Review identity.
    pub review_id: String,
    /// Contract evaluated.
    pub contract_id: String,
    /// Reviewer run.
    pub reviewer_run_id: String,
    /// Submission evaluated.
    pub reviewed_submission_id: String,
    /// Review attempt for the submission.
    pub review_attempt_index: i64,
    /// Prior needs-human review, when any.
    pub supersedes_review_id: Option<String>,
    /// Approve, request_changes, or needs_human.
    pub verdict: GraphqlTaskReviewVerdict,
    /// Safe reviewer feedback.
    pub feedback: String,
    /// Criterion outcomes.
    pub criteria: Vec<GraphqlTaskReviewCriterion>,
    /// Creation timestamp.
    pub created_at: String,
}

impl From<TaskReviewRecord> for GraphqlTaskReview {
    fn from(value: TaskReviewRecord) -> Self {
        Self {
            review_id: value.review_id,
            contract_id: value.contract_id.into_string(),
            reviewer_run_id: value.reviewer_run_id,
            reviewed_submission_id: value.reviewed_submission_id,
            review_attempt_index: i64::from(value.review_attempt_index),
            supersedes_review_id: value.supersedes_review_id,
            verdict: value.overall_verdict.into(),
            feedback: value.overall_feedback,
            criteria: value
                .criteria
                .into_iter()
                .map(|criterion| GraphqlTaskReviewCriterion {
                    criterion_id: criterion.criterion_id,
                    outcome: criterion.outcome.into(),
                    evidence_markdown: criterion.evidence_markdown,
                    feedback: criterion.feedback,
                })
                .collect(),
            created_at: value.created_at,
        }
    }
}

/// Compact review projection used by task cards.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskReviewSummary")]
pub struct GraphqlTaskReviewSummary {
    /// Review identity.
    pub review_id: String,
    /// Submission evaluated.
    pub reviewed_submission_id: String,
    /// Review attempt.
    pub review_attempt_index: i64,
    /// Prior review, when any.
    pub supersedes_review_id: Option<String>,
    /// Verdict.
    pub verdict: GraphqlTaskReviewVerdict,
    /// Safe feedback.
    pub feedback: String,
    /// Creation timestamp.
    pub created_at: String,
}

impl From<TaskReviewRecord> for GraphqlTaskReviewSummary {
    fn from(value: TaskReviewRecord) -> Self {
        Self {
            review_id: value.review_id,
            reviewed_submission_id: value.reviewed_submission_id,
            review_attempt_index: i64::from(value.review_attempt_index),
            supersedes_review_id: value.supersedes_review_id,
            verdict: value.overall_verdict.into(),
            feedback: value.overall_feedback,
            created_at: value.created_at,
        }
    }
}

/// Safe audit projection of one run.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskRun")]
pub struct GraphqlTaskRun {
    /// Run identity.
    pub run_id: String,
    /// Run role.
    pub kind: GraphqlTaskRunKind,
    /// Run-local status.
    pub status: GraphqlTaskRunStatus,
    /// Agent identity.
    pub agent_id: String,
    /// Task generation.
    pub task_generation: i64,
    /// Contract identity, absent for Planner.
    pub contract_id: Option<String>,
    /// Attempt index.
    pub attempt_index: i64,
    /// Review round.
    pub review_round: i64,
    /// Parent run, when any.
    pub parent_run_id: Option<String>,
    /// Submission trigger, when any.
    pub triggering_submission_id: Option<String>,
    /// Review trigger, when any.
    pub triggering_review_id: Option<String>,
    /// Requested model snapshot.
    pub model: GraphqlTaskModelSnapshot,
    /// Safe actual provider family.
    pub actual_provider_kind: Option<String>,
    /// Safe actual model profile.
    pub actual_model_profile: Option<String>,
    /// Immutable policy snapshot.
    pub execution_policy: GraphqlTaskExecutionPolicy,
    /// Safe terminal error code.
    pub error_code: Option<String>,
    /// Safe terminal error message.
    pub error_message: Option<String>,
    /// Completed provider calls.
    pub provider_call_count: i64,
    /// Dispatched tool calls.
    pub tool_call_count: i64,
    /// Cumulative input tokens.
    pub input_tokens: i64,
    /// Cumulative cached-input tokens.
    pub cached_input_tokens: i64,
    /// Cumulative output tokens.
    pub output_tokens: i64,
    /// Active execution duration in milliseconds.
    pub active_milliseconds: i64,
    /// Queue timestamp.
    pub queued_at: String,
    /// Start timestamp.
    pub started_at: Option<String>,
    /// End timestamp.
    pub ended_at: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl TryFrom<AgentRunRecord> for GraphqlTaskRun {
    type Error = async_graphql::Error;

    fn try_from(value: AgentRunRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            run_id: value.run_id,
            kind: value.run_kind.into(),
            status: value.status.into(),
            agent_id: value.agent_id,
            task_generation: exact_u64(value.task_generation)?,
            contract_id: value.contract_id.map(|id| id.into_string()),
            attempt_index: i64::from(value.attempt_index),
            review_round: i64::from(value.review_round),
            parent_run_id: value.parent_run_id,
            triggering_submission_id: value.triggering_submission_id,
            triggering_review_id: value.triggering_review_id,
            model: value.model.into(),
            actual_provider_kind: value.actual_provider_kind,
            actual_model_profile: value.actual_model_profile,
            execution_policy: value.execution_policy.into(),
            error_code: value.error_code,
            error_message: value.error_message,
            provider_call_count: i64::from(value.provider_call_count),
            tool_call_count: i64::from(value.tool_call_count),
            input_tokens: exact_u64(value.input_tokens)?,
            cached_input_tokens: exact_u64(value.cached_input_tokens)?,
            output_tokens: exact_u64(value.output_tokens)?,
            active_milliseconds: exact_u64(value.active_milliseconds)?,
            queued_at: value.queued_at,
            started_at: value.started_at,
            ended_at: value.ended_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

/// Bounded transcript item for a selected run.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskRunItem")]
pub struct GraphqlTaskRunItem {
    /// Item identity.
    pub item_id: String,
    /// Run identity.
    pub run_id: String,
    /// Opaque transcript cursor.
    pub cursor: String,
    /// Sequence index.
    pub sequence_index: i64,
    /// Provider round.
    pub round_index: i64,
    /// Transcript kind.
    pub kind: GraphqlTaskRunItemKind,
    /// Item status.
    pub status: GraphqlTaskRunItemStatus,
    /// Correlation identity, when present.
    pub correlation_id: Option<String>,
    /// Parent item, when present.
    pub parent_item_id: Option<String>,
    /// Safe content text.
    pub content_text: Option<String>,
    /// Safe structured payload.
    pub payload: Json<serde_json::Value>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl GraphqlTaskRunItem {
    pub(super) fn from_edge(cursor: String, value: AgentRunItemRecord) -> Self {
        Self {
            item_id: value.item_id,
            run_id: value.run_id,
            cursor,
            sequence_index: value.sequence_index,
            round_index: value.round_index,
            kind: value.kind.into(),
            status: value.status.into(),
            correlation_id: value.correlation_id,
            parent_item_id: value.parent_item_id,
            content_text: value.content_text,
            payload: Json(value.payload),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
