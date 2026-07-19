use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    TaskContractId, TaskGateKind, TaskId, WorkDomainError,
    error::invalid_input,
    validation::{optional as normalize_optional, required},
};

string_enum! {
/// Overall immutable reviewer disposition.
pub enum TaskReviewVerdict, "review.verdict" {
    /// Every criterion passed.
    Approve => "approve",
    /// One or more criteria failed without uncertainty.
    RequestChanges => "request_changes",
    /// Reviewer needs a structured human gate.
    NeedsHuman => "needs_human",
}
}

string_enum! {
/// Per-criterion reviewer outcome.
pub enum CriterionOutcome, "review.criterion.outcome" {
    /// Criterion is demonstrably satisfied.
    Pass => "pass",
    /// Criterion is not satisfied.
    Fail => "fail",
    /// Evidence is unavailable or contradictory.
    Uncertain => "uncertain",
}
}

/// One immutable criterion outcome in a review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskReviewCriterion {
    pub criterion_id: String,
    pub outcome: CriterionOutcome,
    pub evidence_markdown: Option<String>,
    pub feedback: Option<String>,
}

/// Input for one immutable reviewer decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct NewTaskReview {
    pub review_id: Option<String>,
    pub task_id: TaskId,
    pub contract_id: TaskContractId,
    pub reviewer_run_id: String,
    pub reviewed_submission_id: String,
    pub review_attempt_index: u32,
    pub supersedes_review_id: Option<String>,
    pub overall_verdict: TaskReviewVerdict,
    pub human_gate_kind: Option<TaskGateKind>,
    pub overall_feedback: String,
    pub criteria: Vec<TaskReviewCriterion>,
}

impl NewTaskReview {
    /// Normalize a review and enforce exact criterion/verdict consistency.
    /// # Errors
    /// Returns [`WorkDomainError`] when the attempt index or required text is
    /// invalid, criterion coverage is not exact, or the verdict disagrees with
    /// the criterion outcomes and human-gate selection.
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
/// # Errors
/// Returns [`WorkDomainError`] when approval includes a failed or uncertain
/// criterion, requested changes lack a definite failure, or human review lacks
/// uncertainty and a Clarification or Approval gate.
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
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskReviewRecord {
    pub review_id: String,
    pub task_id: TaskId,
    pub contract_id: TaskContractId,
    pub reviewer_run_id: String,
    pub reviewed_submission_id: String,
    pub review_attempt_index: u32,
    pub supersedes_review_id: Option<String>,
    pub overall_verdict: TaskReviewVerdict,
    pub human_gate_kind: Option<TaskGateKind>,
    pub overall_feedback: String,
    pub criteria: Vec<TaskReviewCriterion>,
    pub created_at: String,
}
