//! Store-owned run fences and typed terminal envelopes.
//!
//! Runtime workers never mutate Work rows directly.  They submit one of these
//! validated envelopes to the command service, which applies the lease,
//! generation, contract, and evidence predicates in one SQLite transaction.

use noema_tasks::{
    AgentRunRecord, NewTaskReview, NewTaskSubmission, NewTaskValidationCriterion, RunKind,
    RunStatus, SafeErrorCode, TaskComplexity, TaskContractId, TaskGateKind, WorkDomainError,
};
use serde::{Deserialize, Serialize};

use crate::{NoemaStore, StoreError};

#[path = "task_controls.rs"]
mod claim;
#[path = "agent_runs/events.rs"]
mod progress;
#[path = "agent_runs/records.rs"]
pub(crate) mod rows;
#[path = "tasks/submissions.rs"]
mod terminals;

pub(crate) use terminals::report_expired_failure_tx;

impl NoemaStore {
    /// Load one run by its exact durable identity without applying a live task
    /// generation or lease fence.
    ///
    /// This read is intentionally historical: cancellation clears a run's
    /// lease and the task may advance generations, but Runtime still needs to
    /// observe the terminal run record that its worker owned.
    ///
    /// # Errors
    ///
    /// Returns an error when the historical run row cannot be decoded.
    pub async fn get_work_run_record(
        &self,
        run_id: &str,
    ) -> Result<Option<AgentRunRecord>, StoreError> {
        let run_id = run_id.trim().to_owned();
        if run_id.is_empty() {
            return Err(StoreError::Work(WorkDomainError::InvalidInput {
                field: "run_id",
                message: "run id cannot be blank".to_string(),
            }));
        }
        self.with_connection(move |connection| {
            let transaction = connection.transaction()?;
            let run = rows::load_run_tx(&transaction, &run_id)?;
            transaction.commit()?;
            Ok(run)
        })
        .await
    }
}

/// Run lease and task-generation fence supplied with every worker write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkRunFence {
    /// Run being advanced.
    pub run_id: String,
    /// Opaque lease token issued by `claim_next_work_run`.
    pub lease_token: String,
    /// Task generation captured by the worker.
    pub task_generation: u64,
    /// Contract expected by Executor/Reviewer terminals.
    pub contract_id: Option<TaskContractId>,
}

impl WorkRunFence {
    /// Validate shape-only fields before opening a transaction.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when a fence identity is blank
    /// or its generation is zero.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        if self.run_id.trim().is_empty() || self.lease_token.trim().is_empty() {
            return Err(WorkDomainError::InvalidInput {
                field: "run_fence",
                message: "run and lease token cannot be blank".to_string(),
            });
        }
        if self.task_generation == 0 {
            return Err(WorkDomainError::InvalidInput {
                field: "run_fence.task_generation",
                message: "generation must be positive".to_string(),
            });
        }
        Ok(())
    }
}

/// One atomic, additive observation of leased run progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkRunProgress {
    /// Exact lease, generation, and contract fence.
    pub fence: WorkRunFence,
    /// Provider that served this call; present exactly when provider calls increment.
    pub actual_provider_kind: Option<String>,
    /// Model that served this call; paired with `actual_provider_kind`.
    pub actual_model_profile: Option<String>,
    /// Number of provider calls completed by this observation.
    pub provider_call_count_delta: u32,
    /// Number of tool calls observed by this progress boundary.
    pub tool_call_count_delta: u32,
    /// Provider-reported input tokens added by this observation.
    pub input_tokens_delta: u64,
    /// Provider-reported cached input tokens added by this observation.
    pub cached_input_tokens_delta: u64,
    /// Provider-reported output tokens added by this observation.
    pub output_tokens_delta: u64,
    /// Active execution time added by this observation.
    pub active_milliseconds_delta: u64,
}

impl WorkRunProgress {
    /// Validate the observation shape before reading durable aggregate values.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when the fence or bounded
    /// progress counters are invalid.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        let actual = match (&self.actual_provider_kind, &self.actual_model_profile) {
            (Some(provider), Some(model)) => {
                validate_actual(provider, "run_progress.actual_provider_kind", 128)?;
                validate_actual(model, "run_progress.actual_model_profile", 256)?;
                true
            }
            (None, None) => false,
            _ => {
                return Err(WorkDomainError::InvalidInput {
                    field: "run_progress.actual_provider",
                    message: "actual provider and model must be supplied together".to_string(),
                });
            }
        };
        if actual != (self.provider_call_count_delta > 0) {
            return Err(WorkDomainError::InvalidInput {
                field: "run_progress.provider_call_count_delta",
                message: "provider identity is required exactly when provider calls increment"
                    .to_string(),
            });
        }
        if self.cached_input_tokens_delta > self.input_tokens_delta {
            return Err(WorkDomainError::InvalidInput {
                field: "run_progress.cached_input_tokens_delta",
                message: "cached input tokens cannot exceed input tokens".to_string(),
            });
        }
        if self.provider_call_count_delta == 0
            && self.tool_call_count_delta == 0
            && self.input_tokens_delta == 0
            && self.cached_input_tokens_delta == 0
            && self.output_tokens_delta == 0
            && self.active_milliseconds_delta == 0
        {
            return Err(WorkDomainError::InvalidInput {
                field: "run_progress",
                message: "progress observation cannot be empty".to_string(),
            });
        }
        Ok(())
    }
}

fn validate_actual(
    value: &str,
    field: &'static str,
    max_len: usize,
) -> Result<(), WorkDomainError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > max_len
        || value.chars().any(char::is_control)
    {
        Err(WorkDomainError::InvalidInput {
            field,
            message: "actual provider/model value is not canonical".to_string(),
        })
    } else {
        Ok(())
    }
}

/// Complete Planner terminal evidence that can be materialized into a
/// Planned immutable execution contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletePlan {
    /// Normalized executable request.
    pub request_markdown: String,
    /// Bounded plan retained in the immutable contract.
    pub execution_plan_markdown: String,
    /// Exact nonempty criterion set.
    pub criteria: Vec<NewTaskValidationCriterion>,
    /// Complexity selected by the planner.
    pub complexity: TaskComplexity,
}

/// Planner can either produce a complete contract or pause at a human gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanTerminal {
    /// Complete immutable plan/criteria.
    Complete(CompletePlan),
    /// Safe blocking question; no contract is created.
    BlockingQuestion {
        /// Human-facing question.
        prompt_markdown: String,
        /// Bounded context that helps answer it.
        context_markdown: String,
        /// Optional direct answers shown to the human.
        suggested_answers: Vec<String>,
        /// Clarification or Approval only.
        gate_kind: TaskGateKind,
    },
}

/// Planner terminal command submitted by the leased runtime worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitPlan {
    /// Run lease/generation fence.
    pub fence: WorkRunFence,
    /// Terminal evidence variant.
    pub terminal: PlanTerminal,
}

impl SubmitPlan {
    /// Validate fields that do not depend on durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when the fence or plan payload
    /// is malformed.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        match &self.terminal {
            PlanTerminal::Complete(plan) => {
                if plan.request_markdown.trim().is_empty()
                    || plan.execution_plan_markdown.trim().is_empty()
                    || plan.criteria.is_empty()
                {
                    return Err(WorkDomainError::InvalidInput {
                        field: "planner.plan",
                        message: "request, execution plan, and criteria are required".to_string(),
                    });
                }
            }
            PlanTerminal::BlockingQuestion {
                prompt_markdown,
                suggested_answers,
                gate_kind,
                ..
            } => {
                if prompt_markdown.trim().is_empty()
                    || !matches!(
                        gate_kind,
                        TaskGateKind::Clarification | TaskGateKind::Approval
                    )
                {
                    return Err(WorkDomainError::InvalidInput {
                        field: "planner.blocking_question",
                        message: "prompt and clarification/approval gate are required".to_string(),
                    });
                }
                validate_suggested_answers(suggested_answers, "planner.blocking_question")?;
            }
        }
        Ok(())
    }
}

/// Executor terminal command containing complete criterion evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitTaskResult {
    /// Run lease/generation/contract fence.
    pub fence: WorkRunFence,
    /// Immutable submission input.  The service resolves artifact snapshots.
    pub submission: NewTaskSubmission,
}

impl SubmitTaskResult {
    /// Validate shape-only evidence fields before durable criterion coverage is
    /// checked against the current contract.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when the fence or submission
    /// evidence envelope is malformed.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        if self.fence.contract_id.is_none() {
            return Err(WorkDomainError::ContractRequired);
        }
        if self.submission.summary.trim().is_empty()
            || self.submission.result_markdown.trim().is_empty()
            || self.submission.criteria.is_empty()
        {
            return Err(WorkDomainError::InvalidInput {
                field: "submission",
                message: "summary, result, and criterion evidence are required".to_string(),
            });
        }
        Ok(())
    }
}

/// Reviewer terminal command containing one immutable outcome per criterion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitTaskReview {
    /// Run lease/generation/contract fence.
    pub fence: WorkRunFence,
    /// Immutable reviewer decision input.
    pub review: NewTaskReview,
}

impl SubmitTaskReview {
    /// Validate shape-only fields; exact criterion/verdict checks happen against
    /// the current contract's criterion IDs in the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when the fence or review
    /// envelope is malformed.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        if self.fence.contract_id.is_none() {
            return Err(WorkDomainError::ContractRequired);
        }
        if self.review.overall_feedback.trim().is_empty() || self.review.criteria.is_empty() {
            return Err(WorkDomainError::InvalidInput {
                field: "review",
                message: "feedback and criterion outcomes are required".to_string(),
            });
        }
        Ok(())
    }
}

/// Executor-safe human gate report; it creates no submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportTaskBlocked {
    /// Run lease/generation/contract fence.
    pub fence: WorkRunFence,
    /// Clarification or Approval gate kind.
    pub gate_kind: TaskGateKind,
    /// Safe prompt shown to the human.
    pub prompt_markdown: String,
    /// Bounded supporting context.
    pub context_markdown: String,
    /// Optional direct answers shown to the human.
    pub suggested_answers: Vec<String>,
}

impl ReportTaskBlocked {
    /// Validate gate shape before the transaction checks current task/run role.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError::InvalidInput`] when the fence, prompt, or
    /// context is malformed.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        if self.fence.contract_id.is_none() {
            return Err(WorkDomainError::ContractRequired);
        }
        if self.prompt_markdown.trim().is_empty()
            || !matches!(
                self.gate_kind,
                TaskGateKind::Clarification | TaskGateKind::Approval
            )
        {
            return Err(WorkDomainError::InvalidInput {
                field: "blocked_gate",
                message: "prompt and clarification/approval gate are required".to_string(),
            });
        }
        validate_suggested_answers(&self.suggested_answers, "blocked_gate")?;
        Ok(())
    }
}

fn validate_suggested_answers(
    answers: &[String],
    field: &'static str,
) -> Result<(), WorkDomainError> {
    if answers.len() > 8
        || answers
            .iter()
            .any(|answer| answer.trim().is_empty() || answer.chars().count() > 1_000)
    {
        return Err(WorkDomainError::InvalidInput {
            field,
            message: "at most eight nonblank answers of 1,000 characters are allowed".to_string(),
        });
    }
    Ok(())
}

/// Safe failure report used by lease-expiry and worker failure recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRunFailure {
    /// Run lease/generation fence.  Lease expiry may use an empty token only
    /// through the store-owned recovery path, never through this envelope.
    pub fence: WorkRunFence,
    /// Terminal/interrupted status to persist.
    pub status: RunStatus,
    /// Closed safe error code retained in the ledger.
    pub error_code: SafeErrorCode,
    /// Redacted bounded diagnostic, never provider payloads or credentials.
    pub error_message: Option<String>,
    /// Whether the same lineage may be retried automatically.
    pub retryable: bool,
}

impl ReportRunFailure {
    /// Validate the failure status and safe diagnostic envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when the fence, terminal status, safe error code, or
    /// bounded diagnostic message is invalid.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        self.fence.validate()?;
        if !matches!(self.status, RunStatus::Interrupted | RunStatus::Failed) {
            return Err(WorkDomainError::InvalidInput {
                field: "run_failure.status",
                message: "failure status must be interrupted or failed".to_string(),
            });
        }
        if self
            .error_message
            .as_deref()
            .is_some_and(|message| message.len() > 1024)
        {
            return Err(WorkDomainError::InvalidInput {
                field: "run_failure.error_message",
                message: "error message is too long".to_string(),
            });
        }
        Ok(())
    }
}

/// Claimed run returned to a worker after a FIFO lease transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimedWorkRun {
    /// Full immutable/durable run projection.
    pub run: AgentRunRecord,
    /// Lease token the worker must echo on every write.
    pub lease_token: String,
}

/// One runtime terminal submission accepted by the service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkRunTerminal {
    /// Planner complete/blocking result.
    Plan(SubmitPlan),
    /// Executor submission result.
    TaskResult(SubmitTaskResult),
    /// Reviewer disposition.
    Review(SubmitTaskReview),
    /// Executor asks for a human gate without a submission.
    Blocked(ReportTaskBlocked),
}

impl WorkRunTerminal {
    /// Validate the envelope before the service opens its transaction.
    ///
    /// # Errors
    ///
    /// Returns the role-specific terminal validation error for malformed input.
    pub fn validate(&self) -> Result<(), WorkDomainError> {
        match self {
            Self::Plan(value) => value.validate(),
            Self::TaskResult(value) => value.validate(),
            Self::Review(value) => value.validate(),
            Self::Blocked(value) => value.validate(),
        }
    }

    /// Return its fixed role without inspecting model text.
    #[must_use]
    pub const fn run_kind(&self) -> RunKind {
        match self {
            Self::Plan(_) => RunKind::Planner,
            Self::TaskResult(_) | Self::Blocked(_) => RunKind::Executor,
            Self::Review(_) => RunKind::Reviewer,
        }
    }
}
