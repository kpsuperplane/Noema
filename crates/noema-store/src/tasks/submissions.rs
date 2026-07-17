use noema_tasks::{
    AgentRunRecord, NewTaskSubmission, SubmissionCriterionEvidence, SubmissionState,
    TASK_REVIEWER_AGENT_ID, TaskSubmissionArtifactRecord, TaskSubmissionRecord, plan_submission,
};
use rusqlite::{OptionalExtension, params};

use super::{
    events::{append_run_event_tx, append_task_event_tx},
    lifecycle::task_domain_error,
};
use crate::{NoemaStore, StoreError, ids::allocate_id};

impl NoemaStore {
    /// Commit an executor submission and queue its reviewer in one transaction.
    pub async fn create_task_submission(
        &self,
        input: NewTaskSubmission,
        lease_token: &str,
    ) -> Result<(TaskSubmissionRecord, AgentRunRecord), StoreError> {
        if lease_token.trim().is_empty() {
            return Err(StoreError::InvariantViolation {
                message: "task submission requires the active run lease".to_string(),
            });
        }
        let task =
            self.get_task(&input.task_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("task not found: {}", input.task_id),
                })?;
        let expected_criterion_ids = self
            .list_task_validation_criteria(&input.task_id)
            .await?
            .into_iter()
            .map(|criterion| criterion.criterion_id)
            .collect::<Vec<_>>();
        let input = input
            .normalized(&expected_criterion_ids)
            .map_err(task_domain_error)?;
        if let Some((submission_id, reviewer_run_id)) = self
            .with_connection(|conn| {
                let tx = conn.transaction()?;
                let replay = existing_submission_replay(&tx, &input)?;
                tx.commit()?;
                Ok(replay)
            })
            .await?
        {
            return load_submission_replay(self, &submission_id, &reviewer_run_id).await;
        }
        let executor_run = self
            .get_agent_run(&input.executor_run_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("executor run not found: {}", input.executor_run_id),
            })?;
        self.validate_task_model_snapshot(&task.reviewer_model)
            .await?;
        let submission_plan = plan_submission(
            input.clone(),
            &expected_criterion_ids,
            SubmissionState {
                task_id: task.task_id.clone(),
                task_status: task.status,
                task_revision_index: task.revision_index,
                latest_run_id: task.latest_run_id.clone(),
                run_id: executor_run.run_id.clone(),
                run_task_id: executor_run.task_id.clone(),
                run_kind: executor_run.run_kind,
                run_status: executor_run.status,
                run_revision_index: executor_run.revision_index,
            },
        )
        .map_err(task_domain_error)?;
        let input = submission_plan.submission.clone();
        let artifacts =
            validate_submission_artifacts(self, &input.task_id, &input.artifact_ids).await?;
        let execution_policy = self.get_task_execution_policy().await?;
        let submission_id = input
            .submission_id
            .clone()
            .unwrap_or_else(|| allocate_id("submission"));
        let reviewer_run_id = allocate_id("run");
        let replay = self.with_connection(|conn| {
            let tx = conn.transaction()?;
            if let Some(replay) = existing_submission_replay(&tx, &input)? {
                tx.commit()?;
                return Ok(Some(replay));
            }
            let persisted_criterion_ids = {
                let mut statement = tx.prepare(
                    "SELECT criterion_id FROM task_validation_criteria WHERE task_id = ?1 ORDER BY criterion_id",
                )?;
                statement
                    .query_map([input.task_id.as_str()], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let persisted_task_state = tx
                .query_row(
                    "SELECT status, revision_index, latest_run_id FROM tasks WHERE task_id = ?1",
                    [&input.task_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("task not found: {}", input.task_id),
                })?;
            let persisted_run_state = tx
                .query_row(
                    "SELECT task_id, run_kind, status, revision_index FROM agent_runs WHERE run_id = ?1",
                    [&input.executor_run_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!("executor run not found: {}", input.executor_run_id),
                })?;
            let persisted_plan = plan_submission(
                input.clone(),
                &persisted_criterion_ids,
                SubmissionState {
                    task_id: input.task_id.clone(),
                    task_status: persisted_task_state.0.parse().map_err(task_domain_error)?,
                    task_revision_index: persisted_task_state.1,
                    latest_run_id: persisted_task_state.2,
                    run_id: input.executor_run_id.clone(),
                    run_task_id: persisted_run_state.0,
                    run_kind: persisted_run_state.1.parse().map_err(task_domain_error)?,
                    run_status: persisted_run_state.2.parse().map_err(task_domain_error)?,
                    run_revision_index: persisted_run_state.3,
                },
            )
            .map_err(task_domain_error)?;
            if persisted_plan != submission_plan {
                return Err(StoreError::InvariantViolation {
                    message: format!("task changed while submitting: {}", input.task_id),
                });
            }
            let fenced = tx.execute(
                "UPDATE agent_runs SET status = 'completed', ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, heartbeat_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE run_id = ?1 AND task_id = ?2 AND run_kind = 'executor' AND revision_index = ?3 AND lease_token = ?4 AND status = 'running' AND cancellation_requested = 0 AND EXISTS (SELECT 1 FROM tasks WHERE task_id = ?2 AND latest_run_id = ?1 AND status IN ('executing', 'revision_requested'))",
                rusqlite::params![input.executor_run_id, input.task_id, input.revision_index, lease_token],
            )?;
            if fenced != 1 {
                return Err(StoreError::InvariantViolation {
                    message: format!(
                        "executor run lease, cancellation, or task state changed while submitting: {}",
                        input.executor_run_id
                    ),
                });
            }
            tx.execute(
                "INSERT INTO task_submissions (submission_id, task_id, executor_run_id, revision_index, summary, result_markdown) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![submission_id, input.task_id, input.executor_run_id, input.revision_index, input.summary.trim(), input.result_markdown.trim()],
            )?;
            for criterion in &input.criteria {
                tx.execute(
                    "INSERT INTO task_submission_criteria (submission_id, criterion_id, evidence_markdown) VALUES (?1, ?2, ?3)",
                    rusqlite::params![submission_id, criterion.criterion_id, criterion.evidence_markdown.trim()],
                )?;
            }
            for artifact in &artifacts {
                tx.execute(
                    "INSERT INTO task_submission_artifacts (submission_id, artifact_id, artifact_version_id) VALUES (?1, ?2, ?3)",
                    rusqlite::params![submission_id, artifact.artifact.artifact_id, artifact.version.artifact_version_id],
                )?;
            }
            tx.execute(
                "INSERT INTO agent_runs (run_id, task_id, run_kind, agent_id, attempt_index, revision_index, triggering_submission_id, provider_kind, provider_account_id, selection_mode, model_profile, reasoning_effort, selection_source, max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, status) VALUES (?1, ?2, 'reviewer', ?3, 0, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'queued')",
                rusqlite::params![
                    reviewer_run_id,
                    task.task_id,
                    TASK_REVIEWER_AGENT_ID,
                    input.revision_index,
                    submission_id,
                    task.reviewer_model.provider_kind,
                    task.reviewer_model.provider_account_id,
                    task.reviewer_model.selection_mode.as_str(),
                    task.reviewer_model.model_profile,
                    task.reviewer_model.reasoning_effort.map(noema_providers::ReasoningEffort::as_persistence_str),
                    task.reviewer_model.selection_source,
                    execution_policy.max_provider_continuations,
                    execution_policy.max_tool_calls,
                    execution_policy.max_active_minutes,
                    execution_policy.progress_audit_interval,
                ],
            )?;
            tx.execute(
                "INSERT INTO run_events (event_id, run_id, sequence_number, event_kind, actor_id, payload_json) VALUES (?1, ?2, 1, 'run.queued', ?3, ?4)",
                rusqlite::params![
                    allocate_id("event"),
                    reviewer_run_id,
                    input.executor_run_id,
                    serde_json::json!({"run_kind": "reviewer", "revision_index": input.revision_index, "triggering_submission_id": submission_id}).to_string(),
                ],
            )?;
            let task_changed = tx.execute(
                "UPDATE tasks SET status = 'reviewing', latest_run_id = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE task_id = ?1 AND latest_run_id = ?3 AND status IN ('executing', 'revision_requested')",
                rusqlite::params![task.task_id, reviewer_run_id, input.executor_run_id],
            )?;
            if task_changed != 1 {
                return Err(StoreError::InvariantViolation {
                    message: format!("task changed while submitting: {}", task.task_id),
                });
            }
            append_run_event_tx(
                &tx,
                &input.executor_run_id,
                "run.completed",
                &input.executor_run_id,
                serde_json::json!({"submission_id": submission_id}),
            )?;
            append_task_event_tx(&tx, &task.task_id, "task.submission_created", &input.executor_run_id, serde_json::json!({"submission_id": submission_id, "reviewer_run_id": reviewer_run_id}))?;
            tx.commit()?;
            Ok(None)
        }).await?;
        if let Some((submission_id, reviewer_run_id)) = replay {
            return load_submission_replay(self, &submission_id, &reviewer_run_id).await;
        }
        let submission = self
            .get_task_submission(&submission_id)
            .await?
            .ok_or_else(|| StoreError::InvariantViolation {
                message: format!("submission disappeared: {submission_id}"),
            })?;
        let run = self.get_agent_run(&reviewer_run_id).await?.ok_or_else(|| {
            StoreError::InvariantViolation {
                message: format!("reviewer run disappeared: {reviewer_run_id}"),
            }
        })?;
        Ok((submission, run))
    }
    /// Return one submission with its criterion evidence.
    pub async fn get_task_submission(
        &self,
        submission_id: &str,
    ) -> Result<Option<TaskSubmissionRecord>, StoreError> {
        let base = self.with_connection(|conn| conn.query_row("SELECT submission_id, task_id, executor_run_id, revision_index, summary, result_markdown, created_at FROM task_submissions WHERE submission_id = ?1 LIMIT 1", [submission_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?))).optional().map_err(StoreError::Sqlite)).await?;
        let Some((
            submission_id,
            task_id,
            executor_run_id,
            revision_index,
            summary,
            result_markdown,
            created_at,
        )) = base
        else {
            return Ok(None);
        };
        let criteria = self.with_connection(|conn| { let mut statement = conn.prepare("SELECT criterion_id, evidence_markdown FROM task_submission_criteria WHERE submission_id = ?1 ORDER BY criterion_id")?; let rows = statement.query_map([submission_id.as_str()], |row| Ok(SubmissionCriterionEvidence { criterion_id: row.get(0)?, evidence_markdown: row.get(1)? }))?; rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite) }).await?;
        let artifact_links = self.with_connection(|conn| { let mut statement = conn.prepare("SELECT artifact_id, artifact_version_id FROM task_submission_artifacts WHERE submission_id = ?1 ORDER BY rowid")?; let rows = statement.query_map([submission_id.as_str()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?; rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Sqlite) }).await?;
        let mut artifacts = Vec::with_capacity(artifact_links.len());
        for (index, (artifact_id, artifact_version_id)) in artifact_links.into_iter().enumerate() {
            let artifact = self.get_artifact(&artifact_id).await?.ok_or_else(|| {
                StoreError::InvariantViolation {
                    message: format!("submission artifact disappeared: {artifact_id}"),
                }
            })?;
            let version = self
                .get_artifact_version(&artifact_version_id)
                .await?
                .ok_or_else(|| StoreError::InvariantViolation {
                    message: format!(
                        "submission artifact version disappeared: {artifact_version_id}"
                    ),
                })?;
            artifacts.push(TaskSubmissionArtifactRecord {
                ordinal: i64::try_from(index + 1).unwrap_or(i64::MAX),
                artifact: artifact.artifact,
                version,
            });
        }
        Ok(Some(TaskSubmissionRecord {
            submission_id,
            task_id,
            executor_run_id,
            revision_index,
            summary,
            result_markdown,
            criteria,
            artifacts,
            created_at,
        }))
    }
}

async fn load_submission_replay(
    store: &NoemaStore,
    submission_id: &str,
    reviewer_run_id: &str,
) -> Result<(TaskSubmissionRecord, AgentRunRecord), StoreError> {
    let submission = store
        .get_task_submission(submission_id)
        .await?
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!("existing submission disappeared: {submission_id}"),
        })?;
    let reviewer_run = store.get_agent_run(reviewer_run_id).await?.ok_or_else(|| {
        StoreError::InvariantViolation {
            message: format!("existing reviewer run disappeared: {reviewer_run_id}"),
        }
    })?;
    Ok((submission, reviewer_run))
}

fn existing_submission_replay(
    conn: &rusqlite::Connection,
    input: &NewTaskSubmission,
) -> Result<Option<(String, String)>, StoreError> {
    let existing = conn
        .query_row(
            "SELECT submission_id, executor_run_id, summary, result_markdown
             FROM task_submissions
             WHERE task_id = ?1 AND revision_index = ?2
             LIMIT 1",
            params![input.task_id, input.revision_index],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((submission_id, executor_run_id, summary, result_markdown)) = existing else {
        return Ok(None);
    };

    let criteria = {
        let mut statement = conn.prepare(
            "SELECT criterion_id, evidence_markdown
             FROM task_submission_criteria
             WHERE submission_id = ?1
             ORDER BY criterion_id",
        )?;
        statement
            .query_map([submission_id.as_str()], |row| {
                Ok(SubmissionCriterionEvidence {
                    criterion_id: row.get(0)?,
                    evidence_markdown: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    let artifact_ids = {
        let mut statement = conn.prepare(
            "SELECT artifact_id
             FROM task_submission_artifacts
             WHERE submission_id = ?1
             ORDER BY rowid",
        )?;
        statement
            .query_map([submission_id.as_str()], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let reviewer_run_ids = {
        let mut statement = conn.prepare(
            "SELECT run_id
             FROM agent_runs
             WHERE task_id = ?1
               AND run_kind = 'reviewer'
               AND triggering_submission_id = ?2
             ORDER BY run_id",
        )?;
        statement
            .query_map(params![input.task_id, submission_id], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?
    };

    let supplied_id_matches = input
        .submission_id
        .as_ref()
        .is_none_or(|supplied| supplied == &submission_id);
    let exact = supplied_id_matches
        && executor_run_id == input.executor_run_id
        && summary == input.summary
        && result_markdown == input.result_markdown
        && criteria == input.criteria
        && artifact_ids == input.artifact_ids;
    if !exact {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "task revision already has a different submission: {} revision {}",
                input.task_id, input.revision_index
            ),
        });
    }
    let [reviewer_run_id] = reviewer_run_ids.as_slice() else {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "existing submission must have exactly one reviewer run: {submission_id}"
            ),
        });
    };
    Ok(Some((submission_id, reviewer_run_id.clone())))
}
async fn validate_submission_artifacts(
    store: &NoemaStore,
    task_id: &str,
    artifact_ids: &[String],
) -> Result<Vec<TaskSubmissionArtifactRecord>, StoreError> {
    if artifact_ids.len() > 100 {
        return Err(StoreError::InvariantViolation {
            message: "task submission cannot link more than 100 artifacts".to_string(),
        });
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut artifacts = Vec::with_capacity(artifact_ids.len());
    for (index, artifact_id) in artifact_ids.iter().enumerate() {
        if artifact_id.trim().is_empty() || !seen.insert(artifact_id.as_str()) {
            return Err(StoreError::InvariantViolation {
                message: "task submission artifact ids must be non-empty and unique".to_string(),
            });
        }
        let artifact = store.get_artifact(artifact_id).await?.ok_or_else(|| {
            StoreError::InvariantViolation {
                message: format!("task submission artifact not found: {artifact_id}"),
            }
        })?;
        if artifact.artifact.owner != noema_artifacts::ArtifactOwnerRef::task(task_id) {
            return Err(StoreError::InvariantViolation {
                message: format!("artifact is not owned by task {task_id}: {artifact_id}"),
            });
        }
        artifacts.push(TaskSubmissionArtifactRecord {
            ordinal: i64::try_from(index + 1).unwrap_or(i64::MAX),
            artifact: artifact.artifact,
            version: artifact.current_version,
        });
    }
    Ok(artifacts)
}
