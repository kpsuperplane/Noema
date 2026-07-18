use std::collections::BTreeSet;

use noema_artifacts::{ArtifactRecord, ArtifactVersionRecord};
use serde::{Deserialize, Serialize};

use crate::{TaskContractId, TaskId, WorkDomainError, error::invalid_input};

/// Criterion evidence attached to an immutable executor submission.
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
    /// Owning task.
    pub task_id: TaskId,
    /// Contract executed by the submission.
    pub contract_id: TaskContractId,
    /// Executor run that produced this submission.
    pub executor_run_id: String,
    /// Review round within the contract.
    pub review_round: u32,
    /// Short result summary.
    pub summary: String,
    /// Complete result Markdown.
    pub result_markdown: String,
    /// Evidence for every immutable criterion.
    pub criteria: Vec<SubmissionCriterionEvidence>,
    /// Task-owned artifacts linked by persistence.
    pub artifact_ids: Vec<String>,
}

impl NewTaskSubmission {
    /// Normalize the submission and enforce exact criterion coverage.
    ///
    /// # Errors
    ///
    /// Returns [`WorkDomainError`] when required text or review round is
    /// invalid, criterion coverage is not exact, or artifact identities are
    /// blank, duplicated, or exceed the submission limit.
    pub fn normalized(&self, expected_criterion_ids: &[String]) -> Result<Self, WorkDomainError> {
        if self.review_round == 0 {
            return Err(invalid_input(
                "submission.review_round",
                "review round must be positive",
            ));
        }
        let summary = required(&self.summary, "submission.summary")?;
        let result_markdown = required(&self.result_markdown, "submission.result_markdown")?;
        let expected = expected_criterion_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        let mut criteria = Vec::with_capacity(self.criteria.len());
        for criterion in &self.criteria {
            let criterion_id = required(&criterion.criterion_id, "submission.criterion_id")?;
            if !seen.insert(criterion_id.clone()) {
                return Err(invalid_input(
                    "submission.criteria",
                    "each criterion must appear exactly once",
                ));
            }
            criteria.push(SubmissionCriterionEvidence {
                criterion_id,
                evidence_markdown: required(&criterion.evidence_markdown, "submission.evidence")?,
            });
        }
        if seen.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
            return Err(invalid_input(
                "submission.criteria",
                "submission must cover every criterion exactly once",
            ));
        }
        if self.artifact_ids.len() > 100 {
            return Err(invalid_input(
                "submission.artifact_ids",
                "at most 100 artifacts may be linked",
            ));
        }
        let mut artifact_ids = Vec::with_capacity(self.artifact_ids.len());
        for artifact_id in &self.artifact_ids {
            let artifact_id = required(artifact_id, "submission.artifact_id")?;
            if artifact_ids.iter().any(|existing| existing == &artifact_id) {
                return Err(invalid_input(
                    "submission.artifact_ids",
                    "artifact ids must be unique",
                ));
            }
            artifact_ids.push(artifact_id);
        }
        criteria.sort_by(|left, right| left.criterion_id.cmp(&right.criterion_id));
        Ok(Self {
            submission_id: normalize_optional(self.submission_id.as_deref()),
            task_id: self.task_id.clone(),
            contract_id: self.contract_id.clone(),
            executor_run_id: required(&self.executor_run_id, "submission.executor_run_id")?,
            review_round: self.review_round,
            summary,
            result_markdown,
            criteria,
            artifact_ids,
        })
    }
}

/// Immutable persisted executor submission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskSubmissionRecord {
    /// Stable submission identity.
    pub submission_id: String,
    /// Owning task.
    pub task_id: TaskId,
    /// Contract executed by the submission.
    pub contract_id: TaskContractId,
    /// Executor run identity.
    pub executor_run_id: String,
    /// Review round within the contract.
    pub review_round: u32,
    /// Short result summary.
    pub summary: String,
    /// Complete result Markdown.
    pub result_markdown: String,
    /// Criterion evidence.
    pub criteria: Vec<SubmissionCriterionEvidence>,
    /// Immutable task-owned artifact snapshots.
    pub artifacts: Vec<TaskSubmissionArtifactRecord>,
    /// Creation timestamp.
    pub created_at: String,
}

/// One immutable artifact-version link captured by a submission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskSubmissionArtifactRecord {
    /// One-based artifact order.
    pub ordinal: u32,
    /// Durable artifact metadata snapshot.
    pub artifact: ArtifactRecord,
    /// Immutable version snapshot.
    pub version: ArtifactVersionRecord,
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
