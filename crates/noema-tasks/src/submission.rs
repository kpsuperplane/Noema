use std::collections::BTreeSet;

use noema_artifacts::{ArtifactRecord, ArtifactVersionRecord};
use serde::{Deserialize, Serialize};

use crate::{
    TaskContractId, TaskId, WorkDomainError,
    error::invalid_input,
    validation::{optional as normalize_optional, required},
};

/// Criterion evidence attached to an immutable executor submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct SubmissionCriterionEvidence {
    pub criterion_id: String,
    pub evidence_markdown: String,
}

/// One verified web source attached to an immutable executor submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskSubmissionCitation {
    pub title: String,
    pub url: String,
    pub start_index: Option<usize>,
    pub end_index: Option<usize>,
}

/// Input for an immutable executor submission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct NewTaskSubmission {
    pub submission_id: Option<String>,
    pub task_id: TaskId,
    pub contract_id: TaskContractId,
    pub executor_run_id: String,
    pub review_round: u32,
    pub summary: String,
    pub result_markdown: String,
    pub citations: Vec<TaskSubmissionCitation>,
    pub criteria: Vec<SubmissionCriterionEvidence>,
    pub artifact_ids: Vec<String>,
}

impl NewTaskSubmission {
    /// Normalize the submission and enforce exact criterion coverage.
    /// # Errors
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
        let mut citations = Vec::with_capacity(self.citations.len());
        for citation in &self.citations {
            if matches!((citation.start_index, citation.end_index), (Some(start), Some(end)) if start >= end)
            {
                return Err(invalid_input(
                    "submission.citations",
                    "citation start index must precede its end index",
                ));
            }
            citations.push(TaskSubmissionCitation {
                title: required(&citation.title, "submission.citation.title")?,
                url: required(&citation.url, "submission.citation.url")?,
                start_index: citation.start_index,
                end_index: citation.end_index,
            });
        }
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
            citations,
            criteria,
            artifact_ids,
        })
    }
}

/// Immutable persisted executor submission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskSubmissionRecord {
    pub submission_id: String,
    pub task_id: TaskId,
    pub contract_id: TaskContractId,
    pub executor_run_id: String,
    pub review_round: u32,
    pub summary: String,
    pub result_markdown: String,
    pub citations: Vec<TaskSubmissionCitation>,
    pub criteria: Vec<SubmissionCriterionEvidence>,
    pub artifacts: Vec<TaskSubmissionArtifactRecord>,
    pub created_at: String,
}

/// One immutable artifact-version link captured by a submission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskSubmissionArtifactRecord {
    pub ordinal: u32,
    pub artifact: ArtifactRecord,
    pub version: ArtifactVersionRecord,
}
