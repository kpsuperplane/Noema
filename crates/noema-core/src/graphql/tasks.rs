//! GraphQL projections and owner-authorized controls for background tasks.

use async_graphql::{Enum, InputObject, Result, SimpleObject};

use crate::{
    AgentRunRecord, ModelConfigSnapshot, TaskComplexity, TaskModelPoolEntry, TaskRecord,
    TaskReviewCriterion, TaskReviewRecord, TaskSubmissionRecord, TaskValidationCriterion,
};

use super::{agents::GraphqlReasoningEffort, errors::graphql_error, schema::GraphqlState};

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
    /// Canonical `executor`, `reviewer`, or `completion_delivery` kind.
    pub run_kind: String,
    /// Built-in agent identity used by the run.
    pub agent_id: String,
    /// Revision represented by this run.
    pub revision_index: i32,
    /// Retry attempt within the revision.
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
    /// Input tokens, when reported.
    pub input_tokens: Option<i32>,
    /// Output tokens, when reported.
    pub output_tokens: Option<i32>,
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
            input_tokens: value.input_tokens.map(|value| value as i32),
            output_tokens: value.output_tokens.map(|value| value as i32),
            queued_at: value.queued_at,
            started_at: value.started_at,
            ended_at: value.ended_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
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

/// Retry the latest failed run while preserving its immutable audit history.
pub(super) async fn retry_task(
    state: &GraphqlState,
    principal_subject: &str,
    task_id: String,
) -> Result<GraphqlTaskDetail> {
    require_local_principal(principal_subject)?;
    let store = state.store()?;
    let (task, _) = store
        .retry_failed_task(task_id.trim(), principal_subject, principal_subject)
        .await
        .map_err(graphql_error)?;
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

/// Replace one executor model-pool entry for the local human.
pub(super) async fn update_task_model_pool_entry(
    state: &GraphqlState,
    principal_subject: &str,
    pool_entry_id: String,
    input: GraphqlTaskModelPoolEntryInput,
) -> Result<GraphqlTaskModelPoolEntry> {
    require_local_principal(principal_subject)?;
    let store = state.store()?;
    let normalized_pool_entry_id = pool_entry_id.trim().to_string();
    store
        .update_task_model_pool_entry(
            &normalized_pool_entry_id,
            crate::NewTaskModelPoolEntry {
                pool_entry_id: Some(normalized_pool_entry_id.clone()),
                complexity: input.complexity.into(),
                label: input.label,
                provider_kind: input.provider_kind,
                provider_account_id: input.provider_account_id,
                model_profile: input.model_profile,
                reasoning_effort: input.reasoning_effort.map(Into::into),
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
            "task model-pool mutation is not authorized",
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
                    modelProfile: "gpt-5.6-mini"
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
    }

    #[tokio::test]
    async fn retry_task_mutation_queues_a_new_attempt() {
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
                "title": "Retry through GraphQL",
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
        store
            .transition_agent_run(
                &run.run_id,
                crate::RunStatus::Failed,
                None,
                Some(("provider_error".to_string(), "model missing".to_string())),
            )
            .await
            .expect("failed run");
        store
            .transition_task(&task_id, crate::TaskStatus::Failed, Some("model missing"))
            .await
            .expect("failed task");
        let schema = crate::graphql::build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute(async_graphql::Request::new(format!(
                "mutation {{ retryTask(taskId: \"{task_id}\") {{ taskId status latestRunId errorMessage runs {{ attemptIndex status }} }} }}"
            )))
            .await
            .into_result()
            .expect("retry mutation");
        let value = response.data.into_json().expect("retry json");

        assert_eq!(value["retryTask"]["status"], "queued");
        assert_eq!(value["retryTask"]["errorMessage"], serde_json::Value::Null);
        assert_eq!(value["retryTask"]["runs"].as_array().map(Vec::len), Some(2));
        assert_eq!(value["retryTask"]["runs"][1]["attemptIndex"], 1);
    }
}
