//! GraphQL projections and owner-authorized controls for background tasks.

use async_graphql::{Enum, InputObject, Json, Result, SimpleObject};

use crate::{
    AgentRunItemRecord, AgentRunRecord, ModelConfigSnapshot, TaskComplexity, TaskExecutionPolicy,
    TaskModelPoolEntry, TaskRecord, TaskReviewCriterion, TaskReviewRecord, TaskSubmissionRecord,
    TaskValidationCriterion,
};

use super::{
    agents::{
        GraphqlReasoningEffort, profiles_from_account, provider_disabled_reason,
        refresh_missing_model_profiles, validate_reasoning_effort_for_profile,
    },
    errors::graphql_error,
    schema::GraphqlState,
};

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

impl From<ModelConfigSnapshot> for GraphqlTaskModelSnapshot {
    fn from(value: ModelConfigSnapshot) -> Self {
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
                        crate::ArtifactVersionStorage::LocalFile { .. } => (
                            Some(crate::artifact_download_url(
                                &linked.version.artifact_version_id,
                            )),
                            None,
                        ),
                        crate::ArtifactVersionStorage::ExternalUrl { url } => (None, Some(url)),
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
    /// Transcript kind, such as `model_input`, `assistant_output`, `tool_call`, or `tool_result`.
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
            kind: value.kind,
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

/// Resolve one owner-authorized task detail projection.
pub(super) async fn task(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
) -> Result<Option<GraphqlTaskDetail>> {
    let store = state.store()?;
    let Some(task) = store
        .get_task(task_id.trim())
        .await
        .map_err(graphql_error)?
    else {
        return Ok(None);
    };
    if task.owner_human_id != principal_subject {
        // Do not reveal whether a task owned by another principal exists.
        return Ok(None);
    }
    detail_from_task(store, task).await.map(Some)
}

/// Resolve one owner-authorized page of a task-run transcript.
pub(super) async fn task_run_items(
    state: &GraphqlState,
    principal_subject: &str,
    run_id: String,
    after: Option<String>,
    first: Option<i32>,
) -> Result<GraphqlTaskRunItemsConnection> {
    let store = state.store()?;
    let run_id = run_id.trim();
    let run = store
        .get_agent_run(run_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("task run is unavailable"))?;
    let is_authorized = store
        .get_task(&run.task_id)
        .await
        .map_err(graphql_error)?
        .is_some_and(|task| task.owner_human_id == principal_subject);
    if !is_authorized {
        return Err(async_graphql::Error::new("task run is unavailable"));
    }

    let first = first.unwrap_or(50);
    if !(1..=100).contains(&first) {
        return Err(async_graphql::Error::new(
            "task run page size must be between 1 and 100",
        ));
    }
    let continuation = after
        .as_deref()
        .map(str::trim)
        .filter(|cursor| !cursor.is_empty())
        .map(|cursor| {
            cursor
                .parse::<i64>()
                .map_err(|_| async_graphql::Error::new("task run transcript cursor is invalid"))
        })
        .transpose()?;
    let page_size = i64::from(first);
    let mut page = store
        .list_agent_run_items_before_page(run_id, continuation, page_size + 1)
        .await
        .map_err(graphql_error)?;
    let has_next_page = page.len() > usize::try_from(page_size).unwrap_or(100);
    if has_next_page {
        page.remove(0);
    }
    let end_cursor = has_next_page
        .then(|| page.first().map(|item| item.sequence_index.to_string()))
        .flatten();
    Ok(GraphqlTaskRunItemsConnection {
        items: page.into_iter().map(Into::into).collect(),
        page_info: GraphqlTaskRunItemsPageInfo {
            end_cursor,
            has_next_page,
        },
    })
}

/// Continue a failed or human-blocked task from its durable context.
pub(super) async fn resume_task(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
    message: Option<String>,
) -> Result<GraphqlTaskDetail> {
    require_local_principal(principal_subject)?;
    let store = state.store()?;
    let (task, _) = store
        .resume_task(
            task_id.trim(),
            principal_subject,
            principal_subject,
            message.as_deref(),
        )
        .await
        .map_err(graphql_error)?;
    state
        .subscriptions()
        .publish_task(crate::graphql::TaskLiveEvent::Changed {
            task_id: task.task_id.clone(),
        });
    detail_from_task(store, task).await
}

/// Cancel one owner-authorized queued, active, or blocked task.
pub(super) async fn cancel_task(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
) -> Result<GraphqlTaskDetail> {
    require_local_principal(principal_subject)?;
    let store = state.store()?;
    let task = store
        .cancel_task(task_id.trim(), principal_subject, principal_subject)
        .await
        .map_err(graphql_error)?;
    state
        .subscriptions()
        .publish_task(crate::graphql::TaskLiveEvent::Changed {
            task_id: task.task_id.clone(),
        });
    let _ = crate::daemon::task_delivery::deliver_task_status_event(
        store,
        state.subscriptions(),
        &task.task_id,
    )
    .await;
    detail_from_task(store, task).await
}

/// Resolve the human-controlled executor model pool.
pub(super) async fn task_model_pools(
    state: &GraphqlState,
    complexity: Option<GraphqlTaskComplexity>,
) -> Result<Vec<GraphqlTaskModelPoolEntry>> {
    let store = state.store()?;
    store
        .list_task_model_pool_settings(complexity.map(TaskComplexity::from))
        .await
        .map_err(graphql_error)
        .map(|entries| entries.into_iter().map(Into::into).collect())
}

/// Resolve global task execution limits shared by every complexity tier.
pub(super) async fn task_execution_policy(
    state: &GraphqlState,
) -> Result<GraphqlTaskExecutionPolicy> {
    state
        .store()?
        .get_task_execution_policy()
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

/// Replace global task execution limits for future and resumed runs.
pub(super) async fn update_task_execution_policy(
    state: &GraphqlState,
    principal_subject: &str,
    input: GraphqlTaskExecutionPolicyInput,
) -> Result<GraphqlTaskExecutionPolicy> {
    require_local_principal(principal_subject)?;
    state
        .store()?
        .update_task_execution_policy(input.into())
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

/// Replace one executor model-pool entry for the local human.
pub(super) async fn update_task_model_pool_entry(
    state: &GraphqlState,
    principal_subject: &str,
    pool_entry_id: String,
    input: GraphqlTaskModelPoolEntryInput,
) -> Result<GraphqlTaskModelPoolEntry> {
    require_local_principal(principal_subject)?;
    let store = state.store()?;
    super::provider_accounts::refresh_foundation_local_availability(state).await;
    let account = store
        .get_provider_account(&input.provider_account_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("provider account not found"))?;
    refresh_missing_model_profiles(state, store, std::slice::from_ref(&account)).await;
    let account = store
        .get_provider_account(&input.provider_account_id)
        .await
        .map_err(graphql_error)?
        .ok_or_else(|| async_graphql::Error::new("provider account not found"))?;
    if !account.is_active || !account.is_default {
        return Err(async_graphql::Error::new(
            "provider account is not selectable",
        ));
    }
    if let Some(reason) = provider_disabled_reason(&account) {
        return Err(async_graphql::Error::new(reason));
    }
    if input.provider_kind != account.provider_kind {
        return Err(async_graphql::Error::new(
            "provider kind does not match provider account",
        ));
    }
    let profiles = profiles_from_account(&account, None);
    let Some(profile) = profiles
        .iter()
        .find(|profile| profile.id == input.model_profile)
    else {
        return Err(async_graphql::Error::new(
            "model profile is not available for provider",
        ));
    };
    let reasoning_effort = validate_reasoning_effort_for_profile(profile, input.reasoning_effort)?;
    let normalized_pool_entry_id = pool_entry_id.trim().to_string();
    store
        .update_task_model_pool_entry(
            &normalized_pool_entry_id,
            crate::NewTaskModelPoolEntry {
                pool_entry_id: Some(normalized_pool_entry_id.clone()),
                complexity: input.complexity.into(),
                label: input.label,
                provider_kind: account.provider_kind,
                provider_account_id: account.provider_account_id,
                model_profile: input.model_profile,
                reasoning_effort,
                enabled: input.enabled,
                sort_order: i64::from(input.sort_order),
            },
        )
        .await
        .map(Into::into)
        .map_err(graphql_error)
}

fn require_local_principal(principal_subject: &str) -> Result<()> {
    if principal_subject == "human:local" {
        Ok(())
    } else {
        Err(async_graphql::Error::new(
            "task operation is not authorized",
        ))
    }
}

async fn detail_from_task(
    store: &crate::NoemaStore,
    task: TaskRecord,
) -> Result<GraphqlTaskDetail> {
    let criteria = store
        .list_task_validation_criteria(&task.task_id)
        .await
        .map_err(graphql_error)?;
    let submissions = store
        .list_task_submissions(&task.task_id)
        .await
        .map_err(graphql_error)?;
    let reviews = store
        .list_task_reviews(&task.task_id)
        .await
        .map_err(graphql_error)?;
    let runs = store
        .list_agent_runs_for_task(&task.task_id)
        .await
        .map_err(graphql_error)?;
    let resumable = matches!(
        task.status,
        crate::TaskStatus::Failed | crate::TaskStatus::WaitingForHuman
    );
    let cancellable = matches!(
        task.status,
        crate::TaskStatus::Queued
            | crate::TaskStatus::Executing
            | crate::TaskStatus::Reviewing
            | crate::TaskStatus::RevisionRequested
            | crate::TaskStatus::WaitingForHuman
    );
    let blocking_question = task.blocked_question.clone();
    Ok(GraphqlTaskDetail {
        task_id: task.task_id,
        title: task.title,
        request_markdown: task.request_markdown,
        complexity: task.complexity.into(),
        status: task.status.as_str().to_string(),
        owner_human_id: task.owner_human_id,
        source: GraphqlTaskSource {
            conversation_id: task.source.conversation_id,
            turn_id: task.source.turn_id,
            item_id: task.source.item_id,
        },
        created_by_agent_id: task.created_by_agent_id,
        creation_tool_call_id: task.creation_tool_call_id,
        pool_entry_id: task.pool_entry_id,
        executor_model: task.executor_model.into(),
        reviewer_model: task.reviewer_model.into(),
        revision_index: task.revision_index as i32,
        max_review_rounds: task.max_review_rounds as i32,
        final_submission_id: task.final_submission_id,
        latest_run_id: task.latest_run_id,
        terminal_reason: task.terminal_reason,
        error_code: task.error_code,
        error_message: task.error_message,
        created_at: task.created_at,
        updated_at: task.updated_at,
        completed_at: task.completed_at,
        resumable,
        cancellable,
        blocking_question,
        criteria: criteria.into_iter().map(Into::into).collect(),
        submissions: submissions.into_iter().map(Into::into).collect(),
        reviews: reviews.into_iter().map(Into::into).collect(),
        runs: runs.into_iter().map(Into::into).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::test_store;

    #[tokio::test]
    async fn task_model_pools_query_projects_canonical_entries() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider");
        let entry = store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("task model settings")
            .into_iter()
            .find(|entry| entry.complexity == TaskComplexity::Simple)
            .expect("simple task model");

        let state = GraphqlState::for_tests_with_store(store);
        let entries = task_model_pools(&state, Some(GraphqlTaskComplexity::Simple))
            .await
            .expect("pool query");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].pool_entry_id, entry.pool_entry_id);
        assert_eq!(entries[0].model_profile, "gpt-5.6-luna");
    }

    #[tokio::test]
    async fn schema_exposes_three_global_settings_and_update_mutation() {
        let store = test_store().await;
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider");
        store
            .update_provider_account_metadata(
                "provider_account:codex:default",
                serde_json::json!({
                    "profiles": [{
                        "id": "gpt-5.6-luna",
                        "label": "GPT-5.6 Luna",
                        "reasoning_efforts": ["medium", "xhigh"],
                        "default_reasoning_effort": "medium"
                    }]
                }),
            )
            .await
            .expect("provider catalog");
        store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("task model settings");
        let schema = crate::graphql::build_schema(
            crate::graphql::GraphqlState::for_tests_with_store(store.clone()),
        );

        let update = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  updateTaskModelPoolEntry(poolEntryId: "task_pool:setting:simple", input: {
                    complexity: SIMPLE
                    label: "Fast"
                    providerKind: "codex"
                    providerAccountId: "provider_account:codex:default"
                    modelProfile: "gpt-5.6-luna"
                    reasoningEffort: MEDIUM
                    enabled: true
                    sortOrder: 0
                  }) {
                    poolEntryId
                    complexity
                    modelProfile
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("update mutation");
        let updated = update.data.into_json().expect("update json");
        assert_eq!(updated["updateTaskModelPoolEntry"]["complexity"], "SIMPLE");

        let query = schema
            .execute(async_graphql::Request::new(
                "{ taskModelPools { poolEntryId modelProfile } }",
            ))
            .await
            .into_result()
            .expect("pool query");
        let queried = query.data.into_json().expect("query json");
        assert_eq!(queried["taskModelPools"].as_array().map(Vec::len), Some(3));
        assert!(!schema.sdl().contains("createTaskModelPoolEntry"));
        assert!(!schema.sdl().contains("deleteTaskModelPoolEntry"));

        let invalid = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  updateTaskModelPoolEntry(poolEntryId: "task_pool:setting:simple", input: {
                    complexity: SIMPLE
                    providerKind: "codex"
                    providerAccountId: "provider_account:codex:default"
                    modelProfile: "model-that-does-not-exist"
                    reasoningEffort: MEDIUM
                    enabled: true
                    sortOrder: 0
                  }) { poolEntryId }
                }
                "#,
            ))
            .await;
        assert_eq!(invalid.errors.len(), 1);
        assert_eq!(
            invalid.errors[0].message,
            "model profile is not available for provider"
        );
    }

    #[tokio::test]
    async fn task_execution_policy_is_global_and_mutable() {
        let store = test_store().await;
        let schema = crate::graphql::build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(
                r#"
                mutation {
                  updateTaskExecutionPolicy(input: {
                    maxProviderContinuations: 64
                    maxToolCalls: 256
                    maxActiveMinutes: 90
                    progressAuditInterval: 16
                  }) {
                    maxProviderContinuations
                    maxToolCalls
                    maxActiveMinutes
                    progressAuditInterval
                  }
                }
                "#,
            ))
            .await
            .into_result()
            .expect("update policy");
        let value = response.data.into_json().expect("policy json");
        assert_eq!(
            value["updateTaskExecutionPolicy"]["maxProviderContinuations"],
            64
        );

        let response = schema
            .execute(async_graphql::Request::new(
                "{ taskExecutionPolicy { maxToolCalls maxActiveMinutes } }",
            ))
            .await
            .into_result()
            .expect("query policy");
        let value = response.data.into_json().expect("policy json");
        assert_eq!(value["taskExecutionPolicy"]["maxToolCalls"], 256);
        assert_eq!(value["taskExecutionPolicy"]["maxActiveMinutes"], 90);
    }

    #[tokio::test]
    async fn resume_task_mutation_queues_a_linked_attempt() {
        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider");
        store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("task model settings");
        let delegated = crate::daemon::task_tool::execute_task_delegate(
            &store,
            &crate::daemon::task_tool::TaskDelegateRuntimeContext {
                conversation_id: "conversation:test".to_string(),
                turn_id: "turn:test".to_string(),
                user_item_id: "item:test".to_string(),
                agent_id: "agent:primary".to_string(),
                provider_kind: "codex".to_string(),
                provider_account_id: "provider_account:codex:default".to_string(),
                model_profile: Some("gpt-5.6-luna".to_string()),
                reasoning_effort: Some(crate::provider::ReasoningEffort::Medium),
            },
            Some("call:test".to_string()),
            &serde_json::json!({
                "title": "Resume through GraphQL",
                "request": "Complete the task",
                "complexity": "simple",
                "executor_model_pool_entry_id": "task_pool:setting:simple",
                "validation_criteria": [{"description": "Completes"}]
            }),
        )
        .await;
        assert!(delegated.success);
        let task_id = delegated.payload["task_id"]
            .as_str()
            .expect("task id")
            .to_string();
        let run = store
            .list_agent_runs_for_task(&task_id)
            .await
            .expect("runs")
            .into_iter()
            .next()
            .expect("executor run");
        let leased = store
            .claim_next_agent_run("worker:test", "lease:test", 120)
            .await
            .expect("claim")
            .expect("leased run");
        assert_eq!(leased.run_id, run.run_id);
        store
            .transition_agent_run(
                &run.run_id,
                crate::RunStatus::Running,
                Some("lease:test"),
                None,
            )
            .await
            .expect("running run");
        for index in 1..=3 {
            store
                .append_agent_run_item(
                    crate::NewAgentRunItem {
                        item_id: Some(format!("run_item:page-{index}")),
                        run_id: run.run_id.clone(),
                        round_index: 0,
                        kind: "assistant_output".to_string(),
                        status: crate::AgentRunItemStatus::Completed,
                        correlation_id: None,
                        parent_item_id: None,
                        content_text: Some(format!("item {index}")),
                        payload: serde_json::json!({"index": index}),
                    },
                    "lease:test",
                )
                .await
                .expect("run item");
        }
        store
            .transition_agent_run(
                &run.run_id,
                crate::RunStatus::Failed,
                Some("lease:test"),
                Some(("provider_error".to_string(), "model missing".to_string())),
            )
            .await
            .expect("failed run");
        let schema = crate::graphql::build_schema(GraphqlState::for_tests_with_store(store));

        let page = schema
            .execute(async_graphql::Request::new(format!(
                "{{ taskRunItems(runId: \"{}\", first: 2) {{ items {{ cursor contentText }} pageInfo {{ endCursor hasNextPage }} }} }}",
                run.run_id
            )))
            .await
            .into_result()
            .expect("task run items");
        let page = page.data.into_json().expect("page json");
        assert_eq!(page["taskRunItems"]["items"][0]["contentText"], "item 2");
        assert_eq!(page["taskRunItems"]["items"][1]["contentText"], "item 3");
        assert_eq!(page["taskRunItems"]["pageInfo"]["endCursor"], "2");
        assert_eq!(page["taskRunItems"]["pageInfo"]["hasNextPage"], true);

        let response = schema
            .execute(async_graphql::Request::new(format!(
                "mutation {{ resumeTask(taskId: \"{task_id}\") {{ taskId status latestRunId errorMessage runs {{ attemptIndex status }} }} }}"
            )))
            .await
            .into_result()
            .expect("resume mutation");
        let value = response.data.into_json().expect("resume json");

        assert_eq!(value["resumeTask"]["status"], "queued");
        assert_eq!(value["resumeTask"]["errorMessage"], serde_json::Value::Null);
        assert_eq!(
            value["resumeTask"]["runs"].as_array().map(Vec::len),
            Some(2)
        );
        assert_eq!(value["resumeTask"]["runs"][1]["attemptIndex"], 1);
    }

    #[tokio::test]
    async fn cancel_task_delivers_one_structured_conversation_event() {
        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        store
            .ensure_default_provider_account()
            .await
            .expect("provider account");
        store
            .update_provider_account_status(
                "provider_account:codex:default",
                crate::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated provider");
        let conversation = store
            .create_conversation(crate::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let pool = store
            .ensure_default_task_model_pool_settings("codex")
            .await
            .expect("task models")
            .into_iter()
            .find(|entry| entry.complexity == TaskComplexity::Simple)
            .expect("simple model");
        let (task, _) = store
            .create_task_with_executor(crate::NewTask {
                task_id: None,
                title: "Cancellable task".to_string(),
                request_markdown: "Stop when asked".to_string(),
                complexity: TaskComplexity::Simple,
                owner_human_id: "human:local".to_string(),
                source: crate::TaskSource {
                    conversation_id: Some(conversation.conversation_id.clone()),
                    turn_id: None,
                    item_id: None,
                },
                created_by_agent_id: "agent:primary".to_string(),
                creation_tool_call_id: None,
                pool_entry_id: pool.pool_entry_id,
                executor_model: pool.model.clone(),
                reviewer_model: pool.model,
                max_review_rounds: None,
                criteria: vec![crate::NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: 1,
                    description: "Stops".to_string(),
                    expected_evidence: None,
                }],
            })
            .await
            .expect("task");
        let schema =
            crate::graphql::build_schema(GraphqlState::for_tests_with_store(store.clone()));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                "mutation {{ cancelTask(taskId: \"{}\") {{ status cancellable }} }}",
                task.task_id
            )))
            .await
            .into_result()
            .expect("cancel mutation");
        let value = response.data.into_json().expect("cancel json");
        assert_eq!(value["cancelTask"]["status"], "cancelled");
        assert_eq!(value["cancelTask"]["cancellable"], false);

        schema
            .execute(async_graphql::Request::new(format!(
                "mutation {{ cancelTask(taskId: \"{}\") {{ status }} }}",
                task.task_id
            )))
            .await
            .into_result()
            .expect("idempotent cancel mutation");

        let delivered = store
            .list_conversation_items(&conversation.conversation_id, crate::ReplayMode::Audit)
            .await
            .expect("conversation items")
            .into_iter()
            .filter(|item| {
                item.kind == crate::ConversationItemKind::TaskReference
                    && item.metadata["source"] == "background_task_status"
            })
            .collect::<Vec<_>>();
        assert_eq!(delivered.len(), 1);
        assert_eq!(delivered[0].payload_json["status"], "cancelled");
    }
}
