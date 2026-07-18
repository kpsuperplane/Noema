use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{TaskContractId, TaskGateKind, TaskId, WorkDomainError, error::invalid_input};

/// Overall immutable reviewer disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskReviewVerdict {
    /// Every criterion passed.
    Approve,
    /// One or more criteria failed without uncertainty.
    RequestChanges,
    /// Reviewer needs a structured human gate.
    NeedsHuman,
}

impl TaskReviewVerdict {
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::RequestChanges => "request_changes",
            Self::NeedsHuman => "needs_human",
        }
    }
}

impl std::fmt::Display for TaskReviewVerdict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskReviewVerdict {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "approve" => Ok(Self::Approve),
            "request_changes" => Ok(Self::RequestChanges),
            "needs_human" => Ok(Self::NeedsHuman),
            other => Err(invalid_input(
                "review.verdict",
                format!("unknown value {other}"),
            )),
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
    /// Stable persisted representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Uncertain => "uncertain",
        }
    }
}

impl std::fmt::Display for CriterionOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for CriterionOutcome {
    type Err = WorkDomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            "uncertain" => Ok(Self::Uncertain),
            other => Err(invalid_input(
                "review.criterion.outcome",
                format!("unknown value {other}"),
            )),
        }
    }
}

/// One immutable criterion outcome in a review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskReviewCriterion {
    /// Criterion identity.
    pub criterion_id: String,
    /// Closed outcome.
    pub outcome: CriterionOutcome,
    /// Evidence considered by the reviewer.
    pub evidence_markdown: Option<String>,
    /// Actionable feedback for an executor revision.
    pub feedback: Option<String>,
}

/// Input for one immutable reviewer decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaskReview {
    /// Optional caller-supplied review id.
    pub review_id: Option<String>,
    /// Owning task.
    pub task_id: TaskId,
    /// Contract evaluated by the review.
    pub contract_id: TaskContractId,
    /// Reviewer run that produced this review.
    pub reviewer_run_id: String,
    /// Submission under review.
    pub reviewed_submission_id: String,
    /// One-based immutable review attempt index for a submission.
    pub review_attempt_index: u32,
    /// Prior needs-human review attempt, when this is a continuation.
    pub supersedes_review_id: Option<String>,
    /// Overall verdict.
    pub overall_verdict: TaskReviewVerdict,
    /// Gate kind when verdict is NeedsHuman.
    pub human_gate_kind: Option<TaskGateKind>,
    /// Safe overall feedback.
    pub overall_feedback: String,
    /// Exactly one outcome per contract criterion.
    pub criteria: Vec<TaskReviewCriterion>,
}

impl NewTaskReview {
    /// Normalize a review and enforce exact criterion/verdict consistency.
    pub fn normalized(&self, expected_criterion_ids: &[String]) -> Result<Self, WorkDomainError> {
        if self.review_attempt_index == 0 {
            return Err(invalid_input(
                "review.review_attempt_index",
                "attempt index must be positive",
            ));
        }
        let expected = expected_criterion_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        let mut criteria = Vec::with_capacity(self.criteria.len());
        for criterion in &self.criteria {
            let criterion_id = required(&criterion.criterion_id, "review.criterion_id")?;
            if !seen.insert(criterion_id.clone()) {
                return Err(invalid_input(
                    "review.criteria",
                    "each criterion must appear exactly once",
                ));
            }
            criteria.push(TaskReviewCriterion {
                criterion_id,
                outcome: criterion.outcome,
                evidence_markdown: normalize_optional(criterion.evidence_markdown.as_deref()),
                feedback: normalize_optional(criterion.feedback.as_deref()),
            });
        }
        if seen.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
            return Err(invalid_input(
                "review.criteria",
                "review must cover every criterion exactly once",
            ));
        }
        validate_review_verdict(self.overall_verdict, self.human_gate_kind, &criteria)?;
        criteria.sort_by(|left, right| left.criterion_id.cmp(&right.criterion_id));
        Ok(Self {
            review_id: normalize_optional(self.review_id.as_deref()),
            task_id: self.task_id.clone(),
            contract_id: self.contract_id.clone(),
            reviewer_run_id: required(&self.reviewer_run_id, "review.reviewer_run_id")?,
            reviewed_submission_id: required(
                &self.reviewed_submission_id,
                "review.reviewed_submission_id",
            )?,
            review_attempt_index: self.review_attempt_index,
            supersedes_review_id: normalize_optional(self.supersedes_review_id.as_deref()),
            overall_verdict: self.overall_verdict,
            human_gate_kind: self.human_gate_kind,
            overall_feedback: required(&self.overall_feedback, "review.overall_feedback")?,
            criteria,
        })
    }
}

/// Validate verdict, criterion outcomes, and human-gate requirements.
pub fn validate_review_verdict(
    verdict: TaskReviewVerdict,
    human_gate_kind: Option<TaskGateKind>,
    criteria: &[TaskReviewCriterion],
) -> Result<(), WorkDomainError> {
    let has_fail = criteria
        .iter()
        .any(|criterion| criterion.outcome == CriterionOutcome::Fail);
    let has_uncertain = criteria
        .iter()
        .any(|criterion| criterion.outcome == CriterionOutcome::Uncertain);
    match verdict {
        TaskReviewVerdict::Approve if has_fail || has_uncertain || human_gate_kind.is_some() => {
            Err(invalid_input(
                "review.verdict",
                "approval requires every criterion to pass and no gate",
            ))
        }
        TaskReviewVerdict::RequestChanges
            if !has_fail || has_uncertain || human_gate_kind.is_some() =>
        {
            Err(invalid_input(
                "review.verdict",
                "request_changes requires a failure and no uncertainty",
            ))
        }
        TaskReviewVerdict::NeedsHuman
            if !has_uncertain
                || !matches!(
                    human_gate_kind,
                    Some(TaskGateKind::Clarification | TaskGateKind::Approval)
                ) =>
        {
            Err(invalid_input(
                "review.verdict",
                "needs_human requires uncertainty and a clarification/approval gate",
            ))
        }
        _ => Ok(()),
    }
}

/// Immutable persisted review with criterion evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskReviewRecord {
    /// Stable review identity.
    pub review_id: String,
    /// Owning task.
    pub task_id: TaskId,
    /// Contract evaluated by the review.
    pub contract_id: TaskContractId,
    /// Reviewer run identity.
    pub reviewer_run_id: String,
    /// Submission evaluated by the review.
    pub reviewed_submission_id: String,
    /// One-based review attempt index.
    pub review_attempt_index: u32,
    /// Prior needs-human review, when any.
    pub supersedes_review_id: Option<String>,
    /// Overall verdict.
    pub overall_verdict: TaskReviewVerdict,
    /// Gate kind for NeedsHuman verdicts.
    pub human_gate_kind: Option<TaskGateKind>,
    /// Safe overall feedback.
    pub overall_feedback: String,
    /// Criterion outcomes.
    pub criteria: Vec<TaskReviewCriterion>,
    /// Creation timestamp.
    pub created_at: String,
}

fn required(value: &str, field: &'static str) -> Result<String, WorkDomainError> {
    let value = value.trim();
    if value.is_empty() {
        Err(invalid_input(field, "value cannot be blank"))
    } else {
        Ok(value.to_string())
    }
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
