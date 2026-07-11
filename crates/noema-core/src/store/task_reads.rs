//! Read-model helpers for task inspection surfaces.

#![allow(clippy::missing_errors_doc)]

use super::{NoemaStore, StoreError};
use crate::{TaskReviewCriterion, TaskReviewRecord, TaskReviewVerdict};

impl NoemaStore {
    /// Return every immutable executor submission for one task in revision
    /// order. The task detail read model uses this instead of exposing raw
    /// run-item payloads to clients.
    pub async fn list_task_submissions(
        &self,
        task_id: &str,
    ) -> Result<Vec<crate::TaskSubmissionRecord>, StoreError> {
        let submission_ids = self
            .with_connection(|conn| {
                let mut statement = conn.prepare(
                    "SELECT submission_id FROM task_submissions WHERE task_id = ?1 ORDER BY revision_index, created_at, submission_id",
                )?;
                let rows = statement.query_map([task_id], |row| row.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;
        let mut submissions = Vec::with_capacity(submission_ids.len());
        for submission_id in submission_ids {
            let submission = self
                .get_task_submission(&submission_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("task submission disappeared: {submission_id}"),
                })?;
            submissions.push(submission);
        }
        Ok(submissions)
    }

    /// Return every immutable adversarial review for one task in creation
    /// order, including its per-criterion evidence and feedback.
    pub async fn list_task_reviews(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskReviewRecord>, StoreError> {
        let reviews = self
            .with_connection(|conn| {
                let mut statement = conn.prepare(
                    "SELECT review_id, task_id, reviewer_run_id, reviewed_submission_id, overall_verdict, overall_feedback, created_at FROM task_reviews WHERE task_id = ?1 ORDER BY created_at, review_id",
                )?;
                let rows = statement.query_map([task_id], |row| {
                    let overall_verdict = row
                        .get::<_, String>(4)?
                        .parse::<TaskReviewVerdict>()
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        overall_verdict,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                })?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::Sqlite)
            })
            .await?;

        let mut output = Vec::with_capacity(reviews.len());
        for (
            review_id,
            review_task_id,
            reviewer_run_id,
            reviewed_submission_id,
            overall_verdict,
            overall_feedback,
            created_at,
        ) in reviews
        {
            let criteria = self
                .with_connection(|conn| {
                    let mut statement = conn.prepare(
                        "SELECT criterion_id, outcome, evidence_markdown, feedback FROM task_review_criteria WHERE review_id = ?1 ORDER BY criterion_id",
                    )?;
                    let rows = statement.query_map([review_id.as_str()], |row| {
                        let outcome = row
                            .get::<_, String>(1)?
                            .parse::<crate::CriterionOutcome>()
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    1,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?;
                        Ok(TaskReviewCriterion {
                            criterion_id: row.get(0)?,
                            outcome,
                            evidence_markdown: row.get(2)?,
                            feedback: row.get(3)?,
                        })
                    })?;
                    rows.collect::<Result<Vec<_>, _>>()
                        .map_err(StoreError::Sqlite)
                })
                .await?;
            output.push(TaskReviewRecord {
                review_id,
                task_id: review_task_id,
                reviewer_run_id,
                reviewed_submission_id,
                overall_verdict,
                overall_feedback,
                criteria,
                created_at,
            });
        }
        Ok(output)
    }
}
