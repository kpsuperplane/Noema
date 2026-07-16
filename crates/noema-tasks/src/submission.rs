use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{TaskDomainError, error::invalid_operation};

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

impl NewTaskSubmission {
    /// Normalize a submission and validate its exact criterion/artifact sets.
    ///
    /// # Errors
    ///
    /// Returns a task-domain error for blank fields, invalid indexes, missing
    /// or duplicate criterion evidence, or invalid artifact-id sets.
    pub fn normalized(&self, expected_criterion_ids: &[String]) -> Result<Self, TaskDomainError> {
        if self.revision_index < 0 {
            return Err(invalid_operation(
                "task submission revision index cannot be negative",
            ));
        }
        let task_id = required(&self.task_id, "submission.task_id")?;
        let executor_run_id = required(&self.executor_run_id, "submission.executor_run_id")?;
        let summary = required(&self.summary, "submission.summary")?;
        let result_markdown = required(&self.result_markdown, "submission.result_markdown")?;

        let expected = expected_criterion_ids
            .iter()
            .map(|value| value.as_str())
            .collect::<BTreeSet<_>>();
        let mut actual = BTreeSet::new();
        let mut criteria = Vec::with_capacity(self.criteria.len());
        for criterion in &self.criteria {
            let criterion_id = required(&criterion.criterion_id, "submission.criterion_id")?;
            let evidence_markdown = required(
                &criterion.evidence_markdown,
                "submission.criterion_evidence",
            )?;
            if !actual.insert(criterion_id.clone()) {
                return Err(invalid_operation(
                    "submission must include each task criterion exactly once",
                ));
            }
            criteria.push(SubmissionCriterionEvidence {
                criterion_id,
                evidence_markdown,
            });
        }
        if actual.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected {
            return Err(invalid_operation(
                "submission must include non-empty evidence for every task criterion exactly once",
            ));
        }
        criteria.sort_by(|left, right| left.criterion_id.cmp(&right.criterion_id));

        if self.artifact_ids.len() > 100 {
            return Err(invalid_operation(
                "task submission cannot link more than 100 artifacts",
            ));
        }
        let mut seen_artifacts = BTreeSet::new();
        let mut artifact_ids = Vec::with_capacity(self.artifact_ids.len());
        for artifact_id in &self.artifact_ids {
            let artifact_id = required(artifact_id, "submission.artifact_id")?;
            if !seen_artifacts.insert(artifact_id.clone()) {
                return Err(invalid_operation(
                    "task submission artifact ids must be non-empty and unique",
                ));
            }
            artifact_ids.push(artifact_id);
        }

        Ok(Self {
            submission_id: optional(self.submission_id.as_ref()),
            task_id,
            executor_run_id,
            revision_index: self.revision_index,
            summary,
            result_markdown,
            criteria,
            artifact_ids,
        })
    }
}

/// Persisted executor submission with criterion evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskSubmissionRecord {
    /// Stable submission id.
    pub submission_id: String,
    /// Owning task id.
    pub task_id: String,
    /// Executor run id.
    pub executor_run_id: String,
    /// Revision index.
    pub revision_index: i64,
    /// Short summary.
    pub summary: String,
    /// Complete result Markdown.
    pub result_markdown: String,
    /// Criterion evidence.
    pub criteria: Vec<SubmissionCriterionEvidence>,
    /// Ordered governed artifact snapshots linked by this submission.
    pub artifacts: Vec<TaskSubmissionArtifactRecord>,
    /// Creation timestamp.
    pub created_at: String,
}

/// One governed artifact snapshot linked to an executor submission.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskSubmissionArtifactRecord {
    /// One-based order supplied by the executor.
    pub ordinal: i64,
    /// Durable artifact metadata.
    pub artifact: noema_artifacts::ArtifactRecord,
    /// Immutable version captured when the submission was committed.
    pub version: noema_artifacts::ArtifactVersionRecord,
}

fn required(value: &str, field: &'static str) -> Result<String, TaskDomainError> {
    let value = value.trim();
    if value.is_empty() {
        Err(TaskDomainError::EmptyField(field))
    } else {
        Ok(value.to_string())
    }
}

fn optional(value: Option<&String>) -> Option<String> {
    value
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submission(criteria: Vec<SubmissionCriterionEvidence>) -> NewTaskSubmission {
        NewTaskSubmission {
            submission_id: None,
            task_id: "task:1".to_string(),
            executor_run_id: "run:1".to_string(),
            revision_index: 0,
            summary: "Done".to_string(),
            result_markdown: "Result".to_string(),
            criteria,
            artifact_ids: Vec::new(),
        }
    }

    #[test]
    fn submission_requires_the_exact_criterion_set() {
        let expected = vec!["criterion:1".to_string(), "criterion:2".to_string()];
        let duplicate = submission(vec![
            SubmissionCriterionEvidence {
                criterion_id: "criterion:1".to_string(),
                evidence_markdown: "one".to_string(),
            },
            SubmissionCriterionEvidence {
                criterion_id: "criterion:1".to_string(),
                evidence_markdown: "again".to_string(),
            },
        ]);
        assert!(duplicate.normalized(&expected).is_err());

        let missing = submission(vec![SubmissionCriterionEvidence {
            criterion_id: "criterion:1".to_string(),
            evidence_markdown: "one".to_string(),
        }]);
        assert!(missing.normalized(&expected).is_err());

        let exact = submission(vec![
            SubmissionCriterionEvidence {
                criterion_id: "criterion:2".to_string(),
                evidence_markdown: " two ".to_string(),
            },
            SubmissionCriterionEvidence {
                criterion_id: "criterion:1".to_string(),
                evidence_markdown: " one ".to_string(),
            },
        ])
        .normalized(&expected)
        .expect("exact criterion set");
        assert_eq!(exact.criteria[0].criterion_id, "criterion:1");
        assert_eq!(exact.criteria[0].evidence_markdown, "one");
    }

    #[test]
    fn submission_bounds_and_deduplicates_artifact_ids() {
        let expected = vec!["criterion:1".to_string()];
        let mut input = submission(vec![SubmissionCriterionEvidence {
            criterion_id: "criterion:1".to_string(),
            evidence_markdown: "evidence".to_string(),
        }]);
        input.artifact_ids = vec!["artifact:1".to_string(), " artifact:1 ".to_string()];
        assert!(input.normalized(&expected).is_err());

        input.artifact_ids = (0..101).map(|index| format!("artifact:{index}")).collect();
        assert!(input.normalized(&expected).is_err());
    }
}
