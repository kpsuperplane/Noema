//! GraphQL projections and owner-authorized controls for background tasks.

use async_graphql::{Enum, InputObject, Json, Result, SimpleObject};
use noema_providers::ProviderSelectionSnapshot;
use noema_tasks::{
    AgentRunItemRecord, AgentRunRecord, TaskComplexity, TaskExecutionPolicy, TaskModelPoolEntry,
    TaskRecord, TaskReviewCriterion, TaskReviewRecord, TaskSubmissionRecord,
    TaskValidationCriterion,
};

use super::{
    agents::{
        GraphqlReasoningEffort, provider_disabled_reason, require_selectable_profile,
        selectable_model_account, selectable_profiles_from_account,
        validate_reasoning_effort_for_profile,
    },
    errors::graphql_error,
    schema::GraphqlState,
};

mod resolvers;

pub(super) use resolvers::*;

/// Complexity tier used by the primary agent when selecting an executor pool
/// entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "TaskComplexity")]
pub enum GraphqlTaskComplexity {
    /// Small, low-risk work.
    Simple,
    /// Typical multi-step work.
    Medium,
    /// Large or reasoning-intensive work.
    Difficult,
}

impl From<TaskComplexity> for GraphqlTaskComplexity {
    fn from(value: TaskComplexity) -> Self {
        match value {
            TaskComplexity::Simple => Self::Simple,
            TaskComplexity::Medium => Self::Medium,
            TaskComplexity::Difficult => Self::Difficult,
        }
    }
}

impl From<GraphqlTaskComplexity> for TaskComplexity {
    fn from(value: GraphqlTaskComplexity) -> Self {
        match value {
            GraphqlTaskComplexity::Simple => Self::Simple,
            GraphqlTaskComplexity::Medium => Self::Medium,
            GraphqlTaskComplexity::Difficult => Self::Difficult,
        }
    }
}

/// Immutable model-selection provenance captured on a task or run.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskModelSnapshot")]
pub struct GraphqlTaskModelSnapshot {
    /// Provider family selected for the run.
    pub provider_kind: String,
    /// Concrete provider account selected for the run.
    pub provider_account_id: String,
    /// `explicit_profile` or `provider_default`.
    pub selection_mode: String,
    /// Provider-specific model/profile when explicitly selected.
    pub model_profile: Option<String>,
    /// Explicit reasoning effort, when present.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    /// Source of the model selection, when recorded.
    pub selection_source: Option<String>,
}

impl From<ProviderSelectionSnapshot> for GraphqlTaskModelSnapshot {
    fn from(value: ProviderSelectionSnapshot) -> Self {
        Self {
            provider_kind: value.provider_kind,
            provider_account_id: value.provider_account_id,
            selection_mode: value.selection_mode.as_str().to_string(),
            model_profile: value.model_profile,
            reasoning_effort: value.reasoning_effort.map(GraphqlReasoningEffort::from),
            selection_source: value.selection_source,
        }
    }
}

/// One immutable criterion that the reviewer must evaluate.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskValidationCriterion")]
pub struct GraphqlTaskValidationCriterion {
    /// Stable criterion id.
    pub criterion_id: String,
    /// One-based display order.
    pub ordinal: i32,
    /// Condition the executor must satisfy.
    pub description: String,
    /// Optional evidence guidance.
    pub expected_evidence: Option<String>,
}

impl From<TaskValidationCriterion> for GraphqlTaskValidationCriterion {
    fn from(value: TaskValidationCriterion) -> Self {
        Self {
            criterion_id: value.criterion_id,
            ordinal: value.ordinal as i32,
            description: value.description,
            expected_evidence: value.expected_evidence,
        }
    }
}

/// Source provenance for a delegated task.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskSource")]
pub struct GraphqlTaskSource {
    /// Source conversation id, when delegated from chat.
    pub conversation_id: Option<String>,
    /// Source turn id, when known.
    pub turn_id: Option<String>,
    /// Source transcript item id, when known.
    pub item_id: Option<String>,
}

/// Evidence attached to one executor submission criterion.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskSubmissionCriterionEvidence")]
pub struct GraphqlTaskSubmissionCriterionEvidence {
    /// Criterion being addressed.
    pub criterion_id: String,
    /// Evidence supplied by the executor.
    pub evidence_markdown: String,
}

/// Governed artifact snapshot returned by one executor submission.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskSubmissionArtifact")]
pub struct GraphqlTaskSubmissionArtifact {
    /// Stable artifact id.
    pub artifact_id: String,
    /// Immutable linked version id.
    pub artifact_version_id: String,
    /// Human-readable title.
    pub title: String,
    /// Product-defined artifact kind.
    pub artifact_kind: String,
    /// Canonical storage kind.
    pub storage_kind: String,
    /// Optional media type.
    pub media_type: Option<String>,
    /// Local download route when stored in Noema.
    pub download_url: Option<String>,
    /// External URL when externally hosted.
    pub external_url: Option<String>,
}

/// Immutable executor output for one revision.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskSubmission")]
pub struct GraphqlTaskSubmission {
    /// Stable submission id.
    pub submission_id: String,
    /// Executor run that produced the submission.
    pub executor_run_id: String,
    /// Revision represented by the submission.
    pub revision_index: i32,
    /// Short result summary.
    pub summary: String,
    /// Complete result Markdown.
    pub result_markdown: String,
    /// Criterion evidence captured with the output.
    pub criteria: Vec<GraphqlTaskSubmissionCriterionEvidence>,
    /// Ordered artifact snapshots attached to the output.
    pub artifacts: Vec<GraphqlTaskSubmissionArtifact>,
    /// Creation timestamp.
    pub created_at: String,
}

impl From<TaskSubmissionRecord> for GraphqlTaskSubmission {
    fn from(value: TaskSubmissionRecord) -> Self {
        Self {
            submission_id: value.submission_id,
            executor_run_id: value.executor_run_id,
            revision_index: value.revision_index as i32,
            summary: value.summary,
            result_markdown: value.result_markdown,
            criteria: value
                .criteria
                .into_iter()
                .map(|criterion| GraphqlTaskSubmissionCriterionEvidence {
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
                        storage_kind: linked.artifact.storage_kind.as_str().to_string(),
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

/// One reviewer criterion outcome.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskReviewCriterion")]
pub struct GraphqlTaskReviewCriterion {
    /// Criterion being evaluated.
    pub criterion_id: String,
    /// Canonical `pass`, `fail`, or `uncertain` outcome.
    pub outcome: String,
    /// Evidence considered by the reviewer.
    pub evidence_markdown: Option<String>,
    /// Feedback for a requested executor revision.
    pub feedback: Option<String>,
}

impl From<TaskReviewCriterion> for GraphqlTaskReviewCriterion {
    fn from(value: TaskReviewCriterion) -> Self {
        Self {
            criterion_id: value.criterion_id,
            outcome: value.outcome.as_str().to_string(),
            evidence_markdown: value.evidence_markdown,
            feedback: value.feedback,
        }
    }
}

/// Immutable adversarial review for one executor submission.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskReview")]
pub struct GraphqlTaskReview {
    /// Stable review id.
    pub review_id: String,
    /// Reviewer run that produced the review.
    pub reviewer_run_id: String,
    /// Submission under review.
    pub reviewed_submission_id: String,
    /// Canonical `approve`, `request_changes`, or `needs_human` verdict.
    pub overall_verdict: String,
    /// Safe overall reviewer feedback.
    pub overall_feedback: String,
    /// Per-criterion outcomes and feedback.
    pub criteria: Vec<GraphqlTaskReviewCriterion>,
    /// Creation timestamp.
    pub created_at: String,
}

impl From<TaskReviewRecord> for GraphqlTaskReview {
    fn from(value: TaskReviewRecord) -> Self {
        Self {
            review_id: value.review_id,
            reviewer_run_id: value.reviewer_run_id,
            reviewed_submission_id: value.reviewed_submission_id,
            overall_verdict: value.overall_verdict.as_str().to_string(),
            overall_feedback: value.overall_feedback,
            criteria: value
                .criteria
                .into_iter()
                .map(GraphqlTaskReviewCriterion::from)
                .collect(),
            created_at: value.created_at,
        }
    }
}

/// Safe audit projection of one executor or reviewer run.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskRun")]
pub struct GraphqlTaskRun {
    /// Stable run id.
    pub run_id: String,
    /// Canonical `executor` or `reviewer` kind.
    pub run_kind: String,
    /// Built-in agent identity used by the run.
    pub agent_id: String,
    /// Revision represented by this run.
    pub revision_index: i32,
    /// Continuation or infrastructure-recovery attempt within the revision.
    pub attempt_index: i32,
    /// Canonical durable run status.
    pub status: String,
    /// Requested model snapshot.
    pub model: GraphqlTaskModelSnapshot,
    /// Provider family that actually answered, when recorded.
    pub actual_provider_kind: Option<String>,
    /// Provider model/profile that actually answered, when recorded.
    pub actual_model_profile: Option<String>,
    /// Submission that triggered a reviewer run, when applicable.
    pub triggering_submission_id: Option<String>,
    /// Review that triggered a revision run, when applicable.
    pub triggering_review_id: Option<String>,
    /// Safe terminal error code.
    pub error_code: Option<String>,
    /// Safe terminal error message.
    pub error_message: Option<String>,
    /// Immutable execution-policy snapshot.
    pub execution_policy: GraphqlTaskExecutionPolicy,
    /// Completed provider calls.
    pub provider_call_count: i32,
    /// Dispatched tool calls.
    pub tool_call_count: i32,
    /// Cumulative input tokens.
    pub input_tokens: i32,
    /// Cumulative cached-input tokens.
    pub cached_input_tokens: i32,
    /// Cumulative output tokens.
    pub output_tokens: i32,
    /// Active execution duration in milliseconds.
    pub active_milliseconds: i32,
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

/// Live transcript/activity item for one background run.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskRunItem")]
pub struct GraphqlTaskRunItem {
    /// Stable item id.
    pub item_id: String,
    /// Owning run id.
    pub run_id: String,
    /// Opaque durable pagination/subscription cursor.
    pub cursor: String,
    /// Monotonic display order within the run.
    pub sequence_index: i32,
    /// Zero-based provider round that emitted the item.
    pub round_index: i32,
    /// Transcript kind, such as `assistant_output`, `tool_call`, or `tool_result`.
    pub kind: String,
    /// Canonical pending/running/completed/failed/cancelled/skipped state.
    pub status: String,
    /// Provider/tool correlation id, when present.
    pub correlation_id: Option<String>,
    /// Parent transcript item id, when present.
    pub parent_item_id: Option<String>,
    /// Human-readable activity text.
    pub content_text: Option<String>,
    /// Structured event payload.
    pub payload: Json<serde_json::Value>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

impl From<AgentRunRecord> for GraphqlTaskRun {
    fn from(value: AgentRunRecord) -> Self {
        Self {
            run_id: value.run_id,
            run_kind: value.run_kind.as_str().to_string(),
            agent_id: value.agent_id,
            revision_index: value.revision_index as i32,
            attempt_index: value.attempt_index as i32,
            status: value.status.as_str().to_string(),
            model: value.model.into(),
            actual_provider_kind: value.actual_provider_kind,
            actual_model_profile: value.actual_model_profile,
            triggering_submission_id: value.triggering_submission_id,
            triggering_review_id: value.triggering_review_id,
            error_code: value.error_code,
            error_message: value.error_message,
            execution_policy: value.execution_policy.into(),
            provider_call_count: i32::try_from(value.provider_call_count).unwrap_or(i32::MAX),
            tool_call_count: i32::try_from(value.tool_call_count).unwrap_or(i32::MAX),
            input_tokens: i32::try_from(value.input_tokens).unwrap_or(i32::MAX),
            cached_input_tokens: i32::try_from(value.cached_input_tokens).unwrap_or(i32::MAX),
            output_tokens: i32::try_from(value.output_tokens).unwrap_or(i32::MAX),
            active_milliseconds: i32::try_from(value.active_milliseconds).unwrap_or(i32::MAX),
            queued_at: value.queued_at,
            started_at: value.started_at,
            ended_at: value.ended_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<AgentRunItemRecord> for GraphqlTaskRunItem {
    fn from(value: AgentRunItemRecord) -> Self {
        Self {
            item_id: value.item_id,
            run_id: value.run_id,
            cursor: value.sequence_index.to_string(),
            sequence_index: i32::try_from(value.sequence_index).unwrap_or(i32::MAX),
            round_index: i32::try_from(value.round_index).unwrap_or(i32::MAX),
            kind: value.kind.as_str().to_string(),
            status: value.status.as_str().to_string(),
            correlation_id: value.correlation_id,
            parent_item_id: value.parent_item_id,
            content_text: value.content_text,
            payload: Json(value.payload),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

/// Cursor metadata for one page of task-run transcript items.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskRunItemsPageInfo")]
pub struct GraphqlTaskRunItemsPageInfo {
    /// Cursor to pass as `after` to load the next older page.
    pub end_cursor: Option<String>,
    /// Whether another older page exists.
    pub has_next_page: bool,
}

/// A newest-first page stream for one task-run transcript.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskRunItemsConnection")]
pub struct GraphqlTaskRunItemsConnection {
    /// Items in chronological display order within this page.
    pub items: Vec<GraphqlTaskRunItem>,
    /// Pagination state for loading the next older page.
    pub page_info: GraphqlTaskRunItemsPageInfo,
}

/// Full read model used by the task detail rail.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskDetail")]
pub struct GraphqlTaskDetail {
    /// Stable task id.
    pub task_id: String,
    /// Human-visible title.
    pub title: String,
    /// Immutable executor request Markdown.
    pub request_markdown: String,
    /// Selected complexity tier.
    pub complexity: GraphqlTaskComplexity,
    /// Canonical task status.
    pub status: String,
    /// Source human owning the task.
    pub owner_human_id: String,
    /// Source provenance.
    pub source: GraphqlTaskSource,
    /// Agent that created the task.
    pub created_by_agent_id: String,
    /// Delegation tool call id, when available.
    pub creation_tool_call_id: Option<String>,
    /// Selected executor pool entry.
    pub pool_entry_id: String,
    /// Immutable executor model snapshot.
    pub executor_model: GraphqlTaskModelSnapshot,
    /// Immutable reviewer model snapshot.
    pub reviewer_model: GraphqlTaskModelSnapshot,
    /// Current revision index.
    pub revision_index: i32,
    /// Maximum number of reviewed submissions.
    pub max_review_rounds: i32,
    /// Approved submission id, when complete.
    pub final_submission_id: Option<String>,
    /// Latest queued/running run id.
    pub latest_run_id: Option<String>,
    /// Safe terminal reason.
    pub terminal_reason: Option<String>,
    /// Safe failure code.
    pub error_code: Option<String>,
    /// Safe failure message.
    pub error_message: Option<String>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
    /// Completion timestamp.
    pub completed_at: Option<String>,
    /// Whether the owner can continue this task from its durable context.
    pub resumable: bool,
    /// Whether the owner can cancel this task.
    pub cancellable: bool,
    /// Blocking question awaiting a human answer, when present.
    pub blocking_question: Option<String>,
    /// Immutable validation criteria.
    pub criteria: Vec<GraphqlTaskValidationCriterion>,
    /// Executor submissions in revision order.
    pub submissions: Vec<GraphqlTaskSubmission>,
    /// Adversarial reviews in creation order.
    pub reviews: Vec<GraphqlTaskReview>,
    /// Safe run history.
    pub runs: Vec<GraphqlTaskRun>,
}

/// One human-controlled executor model-pool entry.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskModelPoolEntry")]
pub struct GraphqlTaskModelPoolEntry {
    /// Stable pool entry id.
    pub pool_entry_id: String,
    /// Complexity tier exposed to the primary agent.
    pub complexity: GraphqlTaskComplexity,
    /// Optional human-facing label.
    pub label: Option<String>,
    /// Provider family for this entry.
    pub provider_kind: String,
    /// Provider account owning the model profile.
    pub provider_account_id: String,
    /// Exact provider model/profile.
    pub model_profile: String,
    /// Optional reasoning effort.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    /// Whether this entry can be selected for new tasks.
    pub enabled: bool,
    /// Human-controlled ordering within its tier.
    pub sort_order: i32,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

/// Input for creating or replacing one human-controlled executor pool entry.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "TaskModelPoolEntryInput")]
pub struct GraphqlTaskModelPoolEntryInput {
    /// Complexity tier exposed to the primary agent.
    pub complexity: GraphqlTaskComplexity,
    /// Optional human-facing label.
    pub label: Option<String>,
    /// Provider family for this entry.
    pub provider_kind: String,
    /// Provider account owning the model profile.
    pub provider_account_id: String,
    /// Exact provider model/profile.
    pub model_profile: String,
    /// Optional reasoning effort.
    pub reasoning_effort: Option<GraphqlReasoningEffort>,
    /// Whether this entry can be selected for new tasks.
    pub enabled: bool,
    /// Human-controlled ordering within its tier.
    pub sort_order: i32,
}

/// Global provider-independent safety limits applied to every task model tier.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskExecutionPolicy")]
pub struct GraphqlTaskExecutionPolicy {
    /// Maximum provider continuations before terminal-only finalization.
    pub max_provider_continuations: i32,
    /// Maximum tool calls before terminal-only finalization.
    pub max_tool_calls: i32,
    /// Maximum active execution time in minutes, excluding queue time.
    pub max_active_minutes: i32,
    /// Continuation interval between progress audits.
    pub progress_audit_interval: i32,
}

/// Input for replacing the global Task Executor safety limits.
#[derive(Clone, Debug, InputObject)]
#[graphql(name = "TaskExecutionPolicyInput")]
pub struct GraphqlTaskExecutionPolicyInput {
    /// Maximum provider continuations before terminal-only finalization.
    pub max_provider_continuations: i32,
    /// Maximum tool calls before terminal-only finalization.
    pub max_tool_calls: i32,
    /// Maximum active execution time in minutes, excluding queue time.
    pub max_active_minutes: i32,
    /// Continuation interval between progress audits.
    pub progress_audit_interval: i32,
}

impl From<TaskExecutionPolicy> for GraphqlTaskExecutionPolicy {
    fn from(value: TaskExecutionPolicy) -> Self {
        Self {
            max_provider_continuations: i32::try_from(value.max_provider_continuations)
                .unwrap_or(i32::MAX),
            max_tool_calls: i32::try_from(value.max_tool_calls).unwrap_or(i32::MAX),
            max_active_minutes: i32::try_from(value.max_active_minutes).unwrap_or(i32::MAX),
            progress_audit_interval: i32::try_from(value.progress_audit_interval)
                .unwrap_or(i32::MAX),
        }
    }
}

impl From<GraphqlTaskExecutionPolicyInput> for TaskExecutionPolicy {
    fn from(value: GraphqlTaskExecutionPolicyInput) -> Self {
        Self {
            max_provider_continuations: i64::from(value.max_provider_continuations),
            max_tool_calls: i64::from(value.max_tool_calls),
            max_active_minutes: i64::from(value.max_active_minutes),
            progress_audit_interval: i64::from(value.progress_audit_interval),
        }
    }
}

impl From<TaskModelPoolEntry> for GraphqlTaskModelPoolEntry {
    fn from(value: TaskModelPoolEntry) -> Self {
        Self {
            pool_entry_id: value.pool_entry_id,
            complexity: value.complexity.into(),
            label: value.label,
            provider_kind: value.model.provider_kind,
            provider_account_id: value.model.provider_account_id,
            model_profile: value.model.model_profile.unwrap_or_default(),
            reasoning_effort: value
                .model
                .reasoning_effort
                .map(GraphqlReasoningEffort::from),
            enabled: value.enabled,
            sort_order: value.sort_order as i32,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

include!("tasks_tests.rs");
