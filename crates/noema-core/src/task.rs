//! Durable background task domain vocabulary.
//!
//! The task subsystem deliberately keeps its state machine and model-selection
//! contracts independent of the worker/runtime.  The store uses these types to
//! validate every persisted transition, while the runtime can build prompts and
//! execution requests from immutable snapshots.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::provider::ReasoningEffort;

/// Stable built-in agent id for background executors.
pub const TASK_EXECUTOR_AGENT_ID: &str = "agent:task-executor";
/// Stable built-in agent id for adversarial reviewers.
pub const TASK_REVIEWER_AGENT_ID: &str = "agent:task-reviewer";
/// Default maximum number of reviewed executor submissions for one task.
pub const DEFAULT_TASK_MAX_REVIEW_ROUNDS: i64 = 3;

/// A bounded complexity tier chosen by the primary agent for a delegated task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskComplexity {
    /// Small, low-risk work.
    Simple,
    /// Typical multi-step work.
    Medium,
    /// Large or reasoning-intensive work.
    Difficult,
}

impl TaskComplexity {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Medium => "medium",
            Self::Difficult => "difficult",
        }
    }
}

impl fmt::Display for TaskComplexity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for TaskComplexity {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "simple" => Ok(Self::Simple),
            "medium" => Ok(Self::Medium),
            "difficult" => Ok(Self::Difficult),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "task_complexity",
                value: other.to_string(),
            }),
        }
    }
}

/// User-visible task workflow state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task and its first executor run have been committed but not claimed.
    Queued,
    /// An executor run currently owns the task lease.
    Executing,
    /// A submission is awaiting independent review.
    Reviewing,
    /// Review requested another executor submission.
    RevisionRequested,
    /// Safe progress requires human clarification, approval, or intervention.
    WaitingForHuman,
    /// A reviewer approved every criterion.
    Completed,
    /// No safe continuation is available.
    Failed,
    /// A human or source deletion policy cancelled the task.
    Cancelled,
}

impl TaskStatus {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Executing => "executing",
            Self::Reviewing => "reviewing",
            Self::RevisionRequested => "revision_requested",
            Self::WaitingForHuman => "waiting_for_human",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Return whether this is a terminal product state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Validate a product state transition.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Queued,
                Self::Executing | Self::WaitingForHuman | Self::Failed | Self::Cancelled
            ) | (
                Self::Executing,
                Self::Reviewing | Self::WaitingForHuman | Self::Failed | Self::Cancelled
            ) | (
                Self::Reviewing,
                Self::RevisionRequested
                    | Self::Completed
                    | Self::WaitingForHuman
                    | Self::Failed
                    | Self::Cancelled
            ) | (
                Self::RevisionRequested,
                Self::Executing | Self::WaitingForHuman | Self::Failed | Self::Cancelled
            ) | (
                Self::WaitingForHuman,
                Self::Queued | Self::Executing | Self::Failed | Self::Cancelled
            ) | (Self::Failed, Self::Queued | Self::Cancelled)
        )
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for TaskStatus {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "executing" => Ok(Self::Executing),
            "reviewing" => Ok(Self::Reviewing),
            "revision_requested" => Ok(Self::RevisionRequested),
            "waiting_for_human" => Ok(Self::WaitingForHuman),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "task_status",
                value: other.to_string(),
            }),
        }
    }
}

/// Kind of durable background run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    /// Produces a structured task submission.
    Executor,
    /// Independently checks one executor submission.
    Reviewer,
    /// Delivers an approved result back into the source conversation.
    CompletionDelivery,
}

impl RunKind {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Executor => "executor",
            Self::Reviewer => "reviewer",
            Self::CompletionDelivery => "completion_delivery",
        }
    }
}

impl fmt::Display for RunKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RunKind {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "executor" => Ok(Self::Executor),
            "reviewer" => Ok(Self::Reviewer),
            "completion_delivery" => Ok(Self::CompletionDelivery),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "run_kind",
                value: other.to_string(),
            }),
        }
    }
}

/// Queue/lease state for a durable agent run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Waiting for a worker to claim the run.
    Queued,
    /// Claimed by a worker but not yet started.
    Leased,
    /// Provider/tool execution is active.
    Running,
    /// Run produced its terminal contract.
    Completed,
    /// Run is paused pending an explicit approval or intervention.
    WaitingForApproval,
    /// Worker lost its lease or process shutdown interrupted execution.
    Interrupted,
    /// Run cannot safely continue.
    Failed,
    /// Run was explicitly cancelled.
    Cancelled,
}

impl RunStatus {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Leased => "leased",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::WaitingForApproval => "waiting_for_approval",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Validate a durable run transition.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Leased | Self::Cancelled | Self::Failed)
                | (
                    Self::Leased,
                    Self::Running | Self::Interrupted | Self::Failed | Self::Cancelled
                )
                | (
                    Self::Running,
                    Self::Completed
                        | Self::WaitingForApproval
                        | Self::Interrupted
                        | Self::Failed
                        | Self::Cancelled
                )
                | (
                    Self::WaitingForApproval,
                    Self::Completed | Self::Failed | Self::Cancelled
                )
                | (
                    Self::Interrupted,
                    Self::Queued | Self::Failed | Self::Cancelled
                )
                | (Self::Failed, Self::Queued | Self::Cancelled)
        )
    }
}

impl fmt::Display for RunStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for RunStatus {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "leased" => Ok(Self::Leased),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "waiting_for_approval" => Ok(Self::WaitingForApproval),
            "interrupted" => Ok(Self::Interrupted),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "run_status",
                value: other.to_string(),
            }),
        }
    }
}

/// Whether a model profile is explicit or is delegated to the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelectionMode {
    /// A concrete provider profile is pinned in the snapshot.
    ExplicitProfile,
    /// The provider chooses its current default model/profile.
    ProviderDefault,
}

impl ModelSelectionMode {
    /// Return the stable SQLite/API representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitProfile => "explicit_profile",
            Self::ProviderDefault => "provider_default",
        }
    }
}

impl fmt::Display for ModelSelectionMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ModelSelectionMode {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "explicit_profile" => Ok(Self::ExplicitProfile),
            "provider_default" => Ok(Self::ProviderDefault),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "model_selection_mode",
                value: other.to_string(),
            }),
        }
    }
}

/// Immutable model-request provenance persisted on tasks and runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfigSnapshot {
    /// Provider family, such as `codex` or `openai`.
    pub provider_kind: String,
    /// Concrete provider account selected for this request.
    pub provider_account_id: String,
    /// Explicit profile versus provider-owned default semantics.
    pub selection_mode: ModelSelectionMode,
    /// Provider-specific model/profile, when explicitly selected.
    pub model_profile: Option<String>,
    /// Explicit reasoning effort, when requested.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Human/system source of the selection, retained for auditability.
    pub selection_source: Option<String>,
}

impl ModelConfigSnapshot {
    /// Construct an explicit model-profile snapshot.
    #[must_use]
    pub fn explicit(
        provider_kind: impl Into<String>,
        provider_account_id: impl Into<String>,
        model_profile: impl Into<String>,
        reasoning_effort: Option<ReasoningEffort>,
        selection_source: Option<String>,
    ) -> Self {
        Self {
            provider_kind: provider_kind.into(),
            provider_account_id: provider_account_id.into(),
            selection_mode: ModelSelectionMode::ExplicitProfile,
            model_profile: Some(model_profile.into()),
            reasoning_effort,
            selection_source,
        }
    }

    /// Construct a snapshot that preserves provider-default model semantics.
    #[must_use]
    pub fn provider_default(
        provider_kind: impl Into<String>,
        provider_account_id: impl Into<String>,
        reasoning_effort: Option<ReasoningEffort>,
        selection_source: Option<String>,
    ) -> Self {
        Self {
            provider_kind: provider_kind.into(),
            provider_account_id: provider_account_id.into(),
            selection_mode: ModelSelectionMode::ProviderDefault,
            model_profile: None,
            reasoning_effort,
            selection_source,
        }
    }

    /// Validate and normalize user/model input at the persistence boundary.
    ///
    /// # Errors
    ///
    /// Returns [`ModelConfigError`] when the provider, account, or selection
    /// mode is malformed.
    pub fn normalized(&self) -> Result<Self, ModelConfigError> {
        let provider_kind = self.provider_kind.trim().to_ascii_lowercase();
        if !matches!(
            provider_kind.as_str(),
            "codex" | "openai" | "foundation_local"
        ) {
            return Err(ModelConfigError::UnsupportedProvider { provider_kind });
        }
        let provider_account_id = self.provider_account_id.trim();
        if provider_account_id.is_empty() {
            return Err(ModelConfigError::EmptyField("provider_account_id"));
        }
        let selection_source = self
            .selection_source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let model_profile = self
            .model_profile
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        match (self.selection_mode, model_profile.as_deref()) {
            (ModelSelectionMode::ExplicitProfile, None) => Err(ModelConfigError::ProfileRequired),
            (ModelSelectionMode::ProviderDefault, Some(_)) => {
                Err(ModelConfigError::ProfileForbidden)
            }
            _ => Ok(Self {
                provider_kind,
                provider_account_id: provider_account_id.to_string(),
                selection_mode: self.selection_mode,
                model_profile,
                reasoning_effort: self.reasoning_effort,
                selection_source,
            }),
        }
    }
}

/// Validation failure for a model snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModelConfigError {
    /// A provider family is not supported by Noema's model plane.
    #[error("unsupported model provider: {provider_kind}")]
    UnsupportedProvider {
        /// Provider family that was rejected.
        provider_kind: String,
    },
    /// A required snapshot field was blank.
    #[error("model snapshot field cannot be empty: {0}")]
    EmptyField(&'static str),
    /// Explicit profile selection omitted its profile.
    #[error("explicit model selection requires a model profile")]
    ProfileRequired,
    /// Provider-default selection must not pin a profile.
    #[error("provider-default model selection cannot include a model profile")]
    ProfileForbidden,
}

/// Immutable validation criterion supplied when a task is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskValidationCriterion {
    /// Stable criterion id.
    pub criterion_id: String,
    /// One-based display/evaluation order.
    pub ordinal: i64,
    /// Precise condition that must be true for approval.
    pub description: String,
    /// Optional evidence or validation method guidance.
    pub expected_evidence: Option<String>,
}

/// Input criterion; the store allocates an id when one is not provided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaskValidationCriterion {
    /// Optional caller-supplied stable criterion id.
    pub criterion_id: Option<String>,
    /// One-based display/evaluation order.
    pub ordinal: i64,
    /// Precise condition that must be true for approval.
    pub description: String,
    /// Optional evidence or validation method guidance.
    pub expected_evidence: Option<String>,
}

/// Provenance linking a task to its originating conversation turn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSource {
    /// Source conversation, if the task was delegated from chat.
    pub conversation_id: Option<String>,
    /// Source turn, if known.
    pub turn_id: Option<String>,
    /// Source transcript item, if known.
    pub item_id: Option<String>,
}

/// Input for an atomic task + first executor run transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTask {
    /// Optional caller-supplied task id.
    pub task_id: Option<String>,
    /// Human-visible short title.
    pub title: String,
    /// Normalized Markdown request for the executor.
    pub request_markdown: String,
    /// Complexity tier used to select the pool entry.
    pub complexity: TaskComplexity,
    /// Human owning the task.
    pub owner_human_id: String,
    /// Source conversation provenance.
    pub source: TaskSource,
    /// Agent that created the task (normally the primary agent).
    pub created_by_agent_id: String,
    /// Provider call id that performed delegation, when available.
    pub creation_tool_call_id: Option<String>,
    /// Exact pool entry chosen by the primary agent.
    pub pool_entry_id: String,
    /// Immutable executor model snapshot.
    pub executor_model: ModelConfigSnapshot,
    /// Immutable reviewer model-request snapshot.
    pub reviewer_model: ModelConfigSnapshot,
    /// Initial review-round bound; defaults to three when omitted.
    pub max_review_rounds: Option<i64>,
    /// Immutable criteria checked by the reviewer.
    pub criteria: Vec<NewTaskValidationCriterion>,
}

impl NewTask {
    /// Normalize fields and validate criteria before entering SQLite.
    ///
    /// # Errors
    ///
    /// Returns [`TaskDomainError`] when the task is malformed.
    pub fn normalized(&self) -> Result<Self, TaskDomainError> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err(TaskDomainError::EmptyField("title"));
        }
        let request_markdown = self.request_markdown.trim();
        if request_markdown.is_empty() {
            return Err(TaskDomainError::EmptyField("request_markdown"));
        }
        for (field, value) in [
            ("owner_human_id", self.owner_human_id.as_str()),
            ("created_by_agent_id", self.created_by_agent_id.as_str()),
            ("pool_entry_id", self.pool_entry_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(TaskDomainError::EmptyField(field));
            }
        }
        let max_review_rounds = self
            .max_review_rounds
            .unwrap_or(DEFAULT_TASK_MAX_REVIEW_ROUNDS);
        if max_review_rounds < 1 {
            return Err(TaskDomainError::InvalidReviewRoundLimit(max_review_rounds));
        }
        if self.criteria.is_empty() {
            return Err(TaskDomainError::NoCriteria);
        }
        let mut criteria = self.criteria.clone();
        criteria.sort_by_key(|criterion| criterion.ordinal);
        let mut previous_ordinal = None;
        let mut descriptions = std::collections::HashSet::with_capacity(criteria.len());
        for criterion in &mut criteria {
            if criterion.ordinal < 1 {
                return Err(TaskDomainError::InvalidCriterionOrdinal(criterion.ordinal));
            }
            if previous_ordinal == Some(criterion.ordinal) {
                return Err(TaskDomainError::DuplicateCriterionOrdinal(
                    criterion.ordinal,
                ));
            }
            previous_ordinal = Some(criterion.ordinal);
            let description = criterion.description.trim();
            if description.is_empty() {
                return Err(TaskDomainError::EmptyField("criterion.description"));
            }
            if !descriptions.insert(description.to_string()) {
                return Err(TaskDomainError::DuplicateCriterionDescription(
                    description.to_string(),
                ));
            }
            criterion.description = description.to_string();
            criterion.expected_evidence = criterion
                .expected_evidence
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }

        Ok(Self {
            task_id: self
                .task_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
            title: title.to_string(),
            request_markdown: request_markdown.to_string(),
            complexity: self.complexity,
            owner_human_id: self.owner_human_id.trim().to_string(),
            source: TaskSource {
                conversation_id: normalize_optional(self.source.conversation_id.as_ref()),
                turn_id: normalize_optional(self.source.turn_id.as_ref()),
                item_id: normalize_optional(self.source.item_id.as_ref()),
            },
            created_by_agent_id: self.created_by_agent_id.trim().to_string(),
            creation_tool_call_id: normalize_optional(self.creation_tool_call_id.as_ref()),
            pool_entry_id: self.pool_entry_id.trim().to_string(),
            executor_model: self
                .executor_model
                .normalized()
                .map_err(TaskDomainError::Model)?,
            reviewer_model: self
                .reviewer_model
                .normalized()
                .map_err(TaskDomainError::Model)?,
            max_review_rounds: Some(max_review_rounds),
            criteria,
        })
    }
}

fn normalize_optional(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// Overall reviewer outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskReviewVerdict {
    /// Every criterion passed and the task may complete.
    Approve,
    /// One or more criteria failed and another submission is requested.
    RequestChanges,
    /// Reviewer cannot safely decide without human input.
    NeedsHuman,
}

impl TaskReviewVerdict {
    /// Return stable persistence text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::RequestChanges => "request_changes",
            Self::NeedsHuman => "needs_human",
        }
    }
}

impl FromStr for TaskReviewVerdict {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "approve" => Ok(Self::Approve),
            "request_changes" => Ok(Self::RequestChanges),
            "needs_human" => Ok(Self::NeedsHuman),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "task_review_verdict",
                value: other.to_string(),
            }),
        }
    }
}

/// Per-criterion reviewer outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionOutcome {
    /// Criterion is demonstrably satisfied.
    Pass,
    /// Criterion is not satisfied.
    Fail,
    /// Evidence is unavailable or contradictory.
    Uncertain,
}

impl CriterionOutcome {
    /// Return stable persistence text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Uncertain => "uncertain",
        }
    }
}

impl FromStr for CriterionOutcome {
    type Err = TaskDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            "uncertain" => Ok(Self::Uncertain),
            other => Err(TaskDomainError::InvalidEnum {
                kind: "criterion_outcome",
                value: other.to_string(),
            }),
        }
    }
}

/// Criterion evidence attached to an executor submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionCriterionEvidence {
    /// Criterion being addressed.
    pub criterion_id: String,
    /// Concise evidence text.
    pub evidence_markdown: String,
}

/// Input for an immutable executor submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaskSubmission {
    /// Optional caller-supplied submission id.
    pub submission_id: Option<String>,
    /// Task being submitted.
    pub task_id: String,
    /// Executor run that produced this submission.
    pub executor_run_id: String,
    /// Revision number represented by this submission.
    pub revision_index: i64,
    /// Short result summary.
    pub summary: String,
    /// Complete result Markdown.
    pub result_markdown: String,
    /// Evidence for each immutable criterion.
    pub criteria: Vec<SubmissionCriterionEvidence>,
    /// Task-owned artifacts linked by repository rows.
    pub artifact_ids: Vec<String>,
}

/// Criterion result in a reviewer submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskReviewCriterion {
    /// Criterion being evaluated.
    pub criterion_id: String,
    /// Fail-closed outcome.
    pub outcome: CriterionOutcome,
    /// Evidence considered by the reviewer.
    pub evidence_markdown: Option<String>,
    /// Actionable feedback for an executor revision.
    pub feedback: Option<String>,
}

/// Input for an immutable reviewer decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaskReview {
    /// Optional caller-supplied review id.
    pub review_id: Option<String>,
    /// Task being reviewed.
    pub task_id: String,
    /// Reviewer run that produced this review.
    pub reviewer_run_id: String,
    /// Submission under review.
    pub reviewed_submission_id: String,
    /// Overall reviewer verdict.
    pub overall_verdict: TaskReviewVerdict,
    /// Safe overall feedback.
    pub overall_feedback: String,
    /// Exactly one outcome per immutable task criterion.
    pub criteria: Vec<TaskReviewCriterion>,
}

/// Domain input or persisted vocabulary was malformed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TaskDomainError {
    /// A closed enum value was not recognized.
    #[error("invalid {kind}: {value}")]
    InvalidEnum {
        /// Closed vocabulary being parsed.
        kind: &'static str,
        /// Stored or supplied value.
        value: String,
    },
    /// A required field is blank.
    #[error("task field cannot be empty: {0}")]
    EmptyField(&'static str),
    /// No validation criteria were provided.
    #[error("task must include at least one validation criterion")]
    NoCriteria,
    /// A criterion was not one-based.
    #[error("criterion ordinal must be positive: {0}")]
    InvalidCriterionOrdinal(i64),
    /// Two criteria share an ordinal.
    #[error("duplicate criterion ordinal: {0}")]
    DuplicateCriterionOrdinal(i64),
    /// Two criteria have the same description.
    #[error("duplicate criterion description: {0}")]
    DuplicateCriterionDescription(String),
    /// A task's review-round bound was not positive.
    #[error("invalid maximum review rounds: {0}")]
    InvalidReviewRoundLimit(i64),
    /// A model snapshot was malformed.
    #[error("invalid task model snapshot: {0}")]
    Model(#[from] ModelConfigError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_transition_matrix_rejects_terminal_reopening() {
        assert!(TaskStatus::Reviewing.can_transition_to(TaskStatus::Completed));
        assert!(!TaskStatus::Completed.can_transition_to(TaskStatus::Queued));
        assert!(RunStatus::Interrupted.can_transition_to(RunStatus::Queued));
        assert!(!RunStatus::Completed.can_transition_to(RunStatus::Running));
    }

    #[test]
    fn snapshots_preserve_explicit_and_provider_default_semantics() {
        let explicit = ModelConfigSnapshot::explicit(
            "Codex",
            "provider_account:codex:default",
            " gpt-5.5 ",
            Some(ReasoningEffort::High),
            Some("pool:simple".to_string()),
        )
        .normalized()
        .expect("explicit snapshot");
        assert_eq!(explicit.provider_kind, "codex");
        assert_eq!(explicit.model_profile.as_deref(), Some("gpt-5.5"));

        let inherited = ModelConfigSnapshot::provider_default(
            "openai",
            "provider_account:openai:default",
            None,
            Some("primary:effective".to_string()),
        );
        assert_eq!(
            inherited
                .normalized()
                .expect("default snapshot")
                .model_profile,
            None
        );
    }

    #[test]
    fn task_normalization_rejects_duplicate_criteria() {
        let task = NewTask {
            task_id: None,
            title: "Example".to_string(),
            request_markdown: "Do the thing".to_string(),
            complexity: TaskComplexity::Simple,
            owner_human_id: "human:local".to_string(),
            source: TaskSource::default(),
            created_by_agent_id: "agent:primary".to_string(),
            creation_tool_call_id: None,
            pool_entry_id: "pool:1".to_string(),
            executor_model: ModelConfigSnapshot::explicit(
                "codex",
                "provider_account:codex:default",
                "gpt-5.5",
                None,
                None,
            ),
            reviewer_model: ModelConfigSnapshot::provider_default(
                "codex",
                "provider_account:codex:default",
                None,
                None,
            ),
            max_review_rounds: None,
            criteria: vec![
                NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: 1,
                    description: "same".to_string(),
                    expected_evidence: None,
                },
                NewTaskValidationCriterion {
                    criterion_id: None,
                    ordinal: 2,
                    description: "same".to_string(),
                    expected_evidence: None,
                },
            ],
        };
        assert!(matches!(
            task.normalized(),
            Err(TaskDomainError::DuplicateCriterionDescription(_))
        ));
    }
}
