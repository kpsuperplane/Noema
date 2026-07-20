use async_graphql::Json;
use noema_tasks::{
    AgentRunItemRecord, AgentRunRecord, TaskGateRecord, TaskReviewRecord, TaskSubmissionRecord,
};

use super::*;

graphql_object_from! { "Task gate category." => pub struct GraphqlTaskGate("TaskGate")
    try_from TaskGateRecord as value {
    "Gate identity." => gate_id: String = value.gate_id.into_string(),
    "Task generation." => task_generation: i64 = exact_u64(value.task_generation)?,
    "Gate kind." => kind: GraphqlTaskGateKind = value.kind.into(),
    "Gate state." => state: GraphqlTaskGateState = value.state.into(),
    "Recovery reason, when any." => recovery_reason: Option<GraphqlTaskRecoveryReason> = value.recovery_reason.map(Into::into),
    "Explicit recovery continuation role, when any." => retry_run_kind: Option<GraphqlTaskRunKind> = value.retry_run_kind.map(Into::into),
    "Human prompt." => prompt: String = value.prompt_markdown,
    "Bounded context." => context_markdown: String = value.context_markdown,
    "Opener actor." => opened_by: String = value.opened_by_actor_id,
    "Opening run, when any." => originating_run_id: Option<String> = value.originating_run_id,
    "Open timestamp." => opened_at: String = value.opened_at,
    "Resolver actor, when resolved." => resolved_by: Option<String> = value.resolved_by_actor_id,
    "Resolution timestamp, when resolved." => resolved_at: Option<String> = value.resolved_at,
    "Resolution message identity, when resolved." => resolution: Option<String> = value.resolution_message_id.map(|id| id.into_string()),
} }

graphql_object_from! { "Human message in the immutable task history." => pub struct GraphqlTaskMessage("TaskMessage")
    from noema_tasks::TaskMessageRecord as value {
    "Message identity." => message_id: String = value.message_id.into_string(),
    "Safe Markdown body." => body_markdown: String = value.body_markdown,
    "Author actor." => author: String = value.author_actor_id,
    "Creation timestamp." => created_at: String = value.created_at,
} }

graphql_object_from! { "Current run projection, intentionally separate from task stage." => pub struct GraphqlCurrentRunSummary("CurrentRunSummary")
    from AgentRunRecord as value {
    "Run identity." => run_id: String = value.run_id,
    "Human-friendly instance identity." => instance_name: String = value.instance_name,
    "Planner, Executor, or Reviewer." => kind: GraphqlTaskRunKind = value.run_kind.into(),
    "Run-local queue/lease status." => status: GraphqlTaskRunStatus = value.status.into(),
    "Lineage attempt." => attempt_index: i64 = i64::from(value.attempt_index),
    "Contract identity, absent for Planner." => contract_id: Option<String> = value.contract_id.map(|id| id.into_string()),
    "Queue timestamp." => queued_at: String = value.queued_at,
    "Start timestamp." => started_at: Option<String> = value.started_at,
    "Last update timestamp." => updated_at: String = value.updated_at,
    "Safe activity label." => activity_label: String = format!("{} {}", value.run_kind.as_str(), value.status.as_str()),
} }

graphql_object_from! { "Submission criterion evidence." => pub struct GraphqlTaskSubmissionCriterion("TaskSubmissionCriterion")
    from noema_tasks::SubmissionCriterionEvidence as value {
    "Criterion identity." => criterion_id: String = value.criterion_id,
    "Evidence Markdown." => evidence_markdown: String = value.evidence_markdown,
} }

graphql_object! { "Immutable artifact link shown with a submission." => pub struct GraphqlTaskSubmissionArtifact("TaskSubmissionArtifact") {
    "Artifact identity." => artifact_id: String,
    "Artifact version identity." => artifact_version_id: String,
    "Artifact title." => title: String,
    "Artifact kind." => artifact_kind: String,
    "Storage kind." => storage_kind: crate::graphql::artifacts::GraphqlArtifactStorageKind,
    "Media type, when present." => media_type: Option<String>,
    "Local download route, when present." => download_url: Option<String>,
    "External URL, when present." => external_url: Option<String>,
} }

graphql_object_from! { "Immutable submission evidence." => pub struct GraphqlTaskSubmission("TaskSubmission")
    from TaskSubmissionRecord as value {
    "Submission identity." => submission_id: String = value.submission_id,
    "Contract evaluated." => contract_id: String = value.contract_id.into_string(),
    "Executor run." => executor_run_id: String = value.executor_run_id,
    "Review round." => review_round: i64 = i64::from(value.review_round),
    "Short summary." => summary: String = value.summary,
    "Complete result Markdown." => result_markdown: String = value.result_markdown,
    "Criterion evidence." => criteria: Vec<GraphqlTaskSubmissionCriterion> = value.criteria.into_iter().map(Into::into).collect(),
    "Linked immutable artifact versions." => artifacts: Vec<GraphqlTaskSubmissionArtifact> = value.artifacts.into_iter().map(submission_artifact).collect(),
    "Creation timestamp." => created_at: String = value.created_at,
} }

fn submission_artifact(
    linked: noema_tasks::TaskSubmissionArtifactRecord,
) -> GraphqlTaskSubmissionArtifact {
    let (download_url, external_url) = match linked.version.storage {
        noema_artifacts::ArtifactVersionStorage::LocalFile { .. } => (
            Some(noema_artifacts::artifact_download_url(
                &linked.version.artifact_version_id,
            )),
            None,
        ),
        noema_artifacts::ArtifactVersionStorage::ExternalUrl { url } => (None, Some(url)),
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
}

graphql_object_from! { "Immutable reviewer criterion outcome." => pub struct GraphqlTaskReviewCriterion("TaskReviewCriterion")
    from noema_tasks::TaskReviewCriterion as value {
    "Criterion identity." => criterion_id: String = value.criterion_id,
    "Pass, fail, or uncertain." => outcome: GraphqlTaskCriterionOutcome = value.outcome.into(),
    "Evidence Markdown." => evidence_markdown: Option<String> = value.evidence_markdown,
    "Reviewer feedback." => feedback: Option<String> = value.feedback,
} }

graphql_object_from! { "Immutable reviewer decision." => pub struct GraphqlTaskReview("TaskReview")
    from TaskReviewRecord as value {
    "Review identity." => review_id: String = value.review_id,
    "Contract evaluated." => contract_id: String = value.contract_id.into_string(),
    "Reviewer run." => reviewer_run_id: String = value.reviewer_run_id,
    "Submission evaluated." => reviewed_submission_id: String = value.reviewed_submission_id,
    "Review attempt for the submission." => review_attempt_index: i64 = i64::from(value.review_attempt_index),
    "Prior needs-human review, when any." => supersedes_review_id: Option<String> = value.supersedes_review_id,
    "Approve, request_changes, or needs_human." => verdict: GraphqlTaskReviewVerdict = value.overall_verdict.into(),
    "Safe reviewer feedback." => feedback: String = value.overall_feedback,
    "Criterion outcomes." => criteria: Vec<GraphqlTaskReviewCriterion> = value.criteria.into_iter().map(Into::into).collect(),
    "Creation timestamp." => created_at: String = value.created_at,
} }

graphql_object_from! { "Compact review projection used by task cards." => pub struct GraphqlTaskReviewSummary("TaskReviewSummary")
    from TaskReviewRecord as value {
    "Review identity." => review_id: String = value.review_id,
    "Submission evaluated." => reviewed_submission_id: String = value.reviewed_submission_id,
    "Review attempt." => review_attempt_index: i64 = i64::from(value.review_attempt_index),
    "Prior review, when any." => supersedes_review_id: Option<String> = value.supersedes_review_id,
    "Verdict." => verdict: GraphqlTaskReviewVerdict = value.overall_verdict.into(),
    "Safe feedback." => feedback: String = value.overall_feedback,
    "Creation timestamp." => created_at: String = value.created_at,
} }

graphql_object_from! { "Safe audit projection of one run." => pub struct GraphqlTaskRun("TaskRun")
    try_from AgentRunRecord as value {
    "Run identity." => run_id: String = value.run_id,
    "Human-friendly instance identity." => instance_name: String = value.instance_name,
    "Run role." => kind: GraphqlTaskRunKind = value.run_kind.into(),
    "Run-local status." => status: GraphqlTaskRunStatus = value.status.into(),
    "Agent identity." => agent_id: String = value.agent_id,
    "Task generation." => task_generation: i64 = exact_u64(value.task_generation)?,
    "Contract identity, absent for Planner." => contract_id: Option<String> = value.contract_id.map(|id| id.into_string()),
    "Attempt index." => attempt_index: i64 = i64::from(value.attempt_index),
    "Review round." => review_round: i64 = i64::from(value.review_round),
    "Parent run, when any." => parent_run_id: Option<String> = value.parent_run_id,
    "Submission trigger, when any." => triggering_submission_id: Option<String> = value.triggering_submission_id,
    "Review trigger, when any." => triggering_review_id: Option<String> = value.triggering_review_id,
    "Requested model snapshot." => model: GraphqlTaskModelSnapshot = value.model.into(),
    "Safe actual provider family." => actual_provider_kind: Option<String> = value.actual_provider_kind,
    "Safe actual model profile." => actual_model_profile: Option<String> = value.actual_model_profile,
    "Immutable policy snapshot." => execution_policy: GraphqlTaskExecutionPolicy = value.execution_policy.into(),
    "Safe terminal error code." => error_code: Option<String> = value.error_code,
    "Safe terminal error message." => error_message: Option<String> = value.error_message,
    "Completed provider calls." => provider_call_count: i64 = i64::from(value.provider_call_count),
    "Dispatched tool calls." => tool_call_count: i64 = i64::from(value.tool_call_count),
    "Cumulative input tokens." => input_tokens: i64 = exact_u64(value.input_tokens)?,
    "Cumulative cached-input tokens." => cached_input_tokens: i64 = exact_u64(value.cached_input_tokens)?,
    "Cumulative output tokens." => output_tokens: i64 = exact_u64(value.output_tokens)?,
    "Active execution duration in milliseconds." => active_milliseconds: i64 = exact_u64(value.active_milliseconds)?,
    "Queue timestamp." => queued_at: String = value.queued_at,
    "Start timestamp." => started_at: Option<String> = value.started_at,
    "End timestamp." => ended_at: Option<String> = value.ended_at,
    "Creation timestamp." => created_at: String = value.created_at,
    "Last update timestamp." => updated_at: String = value.updated_at,
} }

graphql_object_from! { "Bounded transcript item for a selected run." => pub struct GraphqlTaskRunItem("TaskRunItem")
    from (String, AgentRunItemRecord) as value {
    "Item identity." => item_id: String = value.1.item_id,
    "Run identity." => run_id: String = value.1.run_id,
    "Opaque transcript cursor." => cursor: String = value.0,
    "Sequence index." => sequence_index: i64 = value.1.sequence_index,
    "Provider round." => round_index: i64 = value.1.round_index,
    "Transcript kind." => kind: GraphqlTaskRunItemKind = value.1.kind.into(),
    "Item status." => status: GraphqlTaskRunItemStatus = value.1.status.into(),
    "Correlation identity, when present." => correlation_id: Option<String> = value.1.correlation_id,
    "Parent item, when present." => parent_item_id: Option<String> = value.1.parent_item_id,
    "Safe content text." => content_text: Option<String> = value.1.content_text,
    "Safe structured payload." => payload: Json<serde_json::Value> = Json(value.1.payload),
    "Creation timestamp." => created_at: String = value.1.created_at,
    "Last update timestamp." => updated_at: String = value.1.updated_at,
} }
