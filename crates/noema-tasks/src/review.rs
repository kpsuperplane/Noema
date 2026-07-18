use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    TaskDomainError,
    error::invalid_operation,
    validation::{normalize_optional, required},
};

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

task_vocabulary!(TaskReviewVerdict, "task_review_verdict", {
    Approve => "approve",
    RequestChanges => "request_changes",
    NeedsHuman => "needs_human",
});

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

task_vocabulary!(CriterionOutcome, "criterion_outcome", {
    Pass => "pass",
    Fail => "fail",
    Uncertain => "uncertain",
});

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

impl NewTaskReview {
    /// Normalize a review and validate its exact criterion set and verdict.
    ///
    /// # Errors
    ///
    /// Returns a task-domain error for blank identifiers, duplicate or missing
    /// criteria, or a verdict inconsistent with criterion outcomes.
    pub fn normalized(&self, expected_criterion_ids: &[String]) -> Result<Self, TaskDomainError> {
        let expected = expected_criterion_ids
            .iter()
            .map(|value| value.as_str())
            .collect::<BTreeSet<_>>();
        let mut actual = BTreeSet::new();
        let mut criteria = Vec::with_capacity(self.criteria.len());
        for criterion in &self.criteria {
            let criterion_id = required(&criterion.criterion_id, "review.criterion_id")?;
            if !actual.insert(criterion_id.clone()) {
                return Err(invalid_operation(
                    "review must include exactly one result for every task criterion",
                ));
            }
            criteria.push(TaskReviewCriterion {
                criterion_id,
                outcome: criterion.outcome,
                evidence_markdown: normalize_optional(criterion.evidence_markdown.as_ref()),
                feedback: normalize_optional(criterion.feedback.as_ref()),
            });
        }
        if actual.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
            return Err(invalid_operation(
                "review must include exactly one result for every task criterion",
            ));
        }
        validate_review_verdict(self.overall_verdict, &criteria)?;
        criteria.sort_by(|left, right| left.criterion_id.cmp(&right.criterion_id));

        Ok(Self {
            review_id: normalize_optional(self.review_id.as_ref()),
            task_id: required(&self.task_id, "review.task_id")?,
            reviewer_run_id: required(&self.reviewer_run_id, "review.reviewer_run_id")?,
            reviewed_submission_id: required(
                &self.reviewed_submission_id,
                "review.reviewed_submission_id",
            )?,
            overall_verdict: self.overall_verdict,
            overall_feedback: required(&self.overall_feedback, "review.overall_feedback")?,
            criteria,
        })
    }
}

pub(crate) fn validate_review_verdict(
    verdict: TaskReviewVerdict,
    criteria: &[TaskReviewCriterion],
) -> Result<(), TaskDomainError> {
    let has_fail = criteria
        .iter()
        .any(|criterion| criterion.outcome == CriterionOutcome::Fail);
    let has_uncertain = criteria
        .iter()
        .any(|criterion| criterion.outcome == CriterionOutcome::Uncertain);
    match verdict {
        TaskReviewVerdict::Approve if has_fail || has_uncertain => Err(invalid_operation(
            "review approval requires every criterion to pass",
        )),
        TaskReviewVerdict::RequestChanges if !has_fail || has_uncertain => Err(invalid_operation(
            "review changes require a failed criterion and no uncertainty",
        )),
        TaskReviewVerdict::NeedsHuman if !has_uncertain => Err(invalid_operation(
            "review human escalation requires an uncertain criterion",
        )),
        _ => Ok(()),
    }
}

/// Persisted adversarial review with per-criterion outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReviewRecord {
    /// Stable review id.
    pub review_id: String,
    /// Owning task id.
    pub task_id: String,
    /// Reviewer run id.
    pub reviewer_run_id: String,
    /// Submission under review.
    pub reviewed_submission_id: String,
    /// Overall verdict.
    pub overall_verdict: TaskReviewVerdict,
    /// Safe overall feedback.
    pub overall_feedback: String,
    /// Criterion outcomes.
    pub criteria: Vec<TaskReviewCriterion>,
    /// Creation timestamp.
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review(verdict: TaskReviewVerdict, criteria: Vec<TaskReviewCriterion>) -> NewTaskReview {
        NewTaskReview {
            review_id: None,
            task_id: "task:1".to_string(),
            reviewer_run_id: "run:reviewer".to_string(),
            reviewed_submission_id: "submission:1".to_string(),
            overall_verdict: verdict,
            overall_feedback: "Reviewed".to_string(),
            criteria,
        }
    }

    fn criterion(id: &str, outcome: CriterionOutcome) -> TaskReviewCriterion {
        TaskReviewCriterion {
            criterion_id: id.to_string(),
            outcome,
            evidence_markdown: None,
            feedback: None,
        }
    }

    #[test]
    fn review_requires_exact_criteria_and_consistent_verdicts() {
        let expected = vec!["criterion:1".to_string()];
        assert!(
            review(
                TaskReviewVerdict::Approve,
                vec![criterion("criterion:1", CriterionOutcome::Fail)]
            )
            .normalized(&expected)
            .is_err()
        );
        assert!(
            review(
                TaskReviewVerdict::RequestChanges,
                vec![criterion("criterion:1", CriterionOutcome::Pass)]
            )
            .normalized(&expected)
            .is_err()
        );
        assert!(
            review(
                TaskReviewVerdict::NeedsHuman,
                vec![criterion("criterion:2", CriterionOutcome::Uncertain)]
            )
            .normalized(&expected)
            .is_err()
        );
        assert!(
            review(
                TaskReviewVerdict::NeedsHuman,
                vec![criterion("criterion:1", CriterionOutcome::Uncertain)]
            )
            .normalized(&expected)
            .is_ok()
        );
        assert!(
            review(
                TaskReviewVerdict::RequestChanges,
                vec![criterion("criterion:1", CriterionOutcome::Uncertain)]
            )
            .normalized(&expected)
            .is_err()
        );
        assert!(
            review(
                TaskReviewVerdict::NeedsHuman,
                vec![criterion("criterion:1", CriterionOutcome::Pass)]
            )
            .normalized(&expected)
            .is_err()
        );
    }
}
