//! Fixed-query hydration for bounded immutable submission pages.

use std::collections::HashMap;

use noema_tasks::{
    NewTaskSubmission, SubmissionCriterionEvidence, TaskContractId, TaskId,
    TaskSubmissionArtifactRecord, TaskSubmissionRecord,
};
use rusqlite::{Row, Transaction, types::Type};

use crate::{StoreError, sqlite::conversion_failure};

struct SubmissionBase {
    submission_id: String,
    task_id: TaskId,
    contract_id: TaskContractId,
    executor_run_id: String,
    review_round: u32,
    summary: String,
    result_markdown: String,
    created_at: String,
}

struct ArtifactLink {
    submission_id: String,
    ordinal: u32,
    artifact_id: String,
    version_id: String,
}

/// Hydrate a submission page with criteria and artifact snapshots in fixed batches.
pub(crate) fn load_submissions(
    transaction: &Transaction<'_>,
    ids: &[String],
) -> Result<HashMap<String, TaskSubmissionRecord>, StoreError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids_json = serde_json::to_string(ids)?;
    let bases = load_bases(transaction, &ids_json)?;
    if bases.len() != ids.len() {
        return Err(invariant(
            "submission history references a missing submission",
        ));
    }
    let criteria = load_criteria(transaction, &ids_json)?;
    let expected = load_expected_criteria(transaction, &bases)?;
    let links = load_artifact_links(transaction, &ids_json)?;
    let artifacts = load_artifacts(transaction, &links)?;
    let versions = load_versions(transaction, &links)?;
    let mut records = HashMap::with_capacity(bases.len());
    for base in bases {
        let submission_criteria = criteria
            .get(&base.submission_id)
            .cloned()
            .unwrap_or_default();
        let expected_criteria = expected
            .get(base.contract_id.as_str())
            .ok_or_else(|| invariant("submission references a missing contract"))?;
        let submission_links = links
            .iter()
            .filter(|link| link.submission_id == base.submission_id)
            .map(|link| {
                let artifact = artifacts
                    .get(&link.artifact_id)
                    .cloned()
                    .ok_or_else(|| invariant("submission artifact is missing"))?;
                let version = versions
                    .get(&link.version_id)
                    .cloned()
                    .ok_or_else(|| invariant("submission artifact version is missing"))?;
                if artifact.owner.object_type != "task"
                    || artifact.owner.object_id != base.task_id.as_str()
                    || version.artifact_id != artifact.artifact_id
                {
                    return Err(invariant(
                        "submission artifact crosses an ownership or version link",
                    ));
                }
                Ok(TaskSubmissionArtifactRecord {
                    ordinal: link.ordinal,
                    artifact,
                    version,
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        let input = NewTaskSubmission {
            submission_id: Some(base.submission_id.clone()),
            task_id: base.task_id.clone(),
            contract_id: base.contract_id.clone(),
            executor_run_id: base.executor_run_id.clone(),
            review_round: base.review_round,
            summary: base.summary.clone(),
            result_markdown: base.result_markdown.clone(),
            criteria: submission_criteria.clone(),
            artifact_ids: submission_links
                .iter()
                .map(|link| link.artifact.artifact_id.clone())
                .collect(),
        };
        if input
            .normalized(expected_criteria)
            .map_err(StoreError::Work)?
            != input
            || base.created_at.trim().is_empty()
        {
            return Err(invariant("submission row is not canonically normalized"));
        }
        let id = base.submission_id.clone();
        let record = TaskSubmissionRecord {
            submission_id: base.submission_id,
            task_id: base.task_id,
            contract_id: base.contract_id,
            executor_run_id: base.executor_run_id,
            review_round: base.review_round,
            summary: base.summary,
            result_markdown: base.result_markdown,
            criteria: submission_criteria,
            artifacts: submission_links,
            created_at: base.created_at,
        };
        if records.insert(id, record).is_some() {
            return Err(invariant("duplicate submission in history batch"));
        }
    }
    Ok(records)
}

fn load_bases(
    transaction: &Transaction<'_>,
    ids_json: &str,
) -> Result<Vec<SubmissionBase>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT submission_id, task_id, contract_id, executor_run_id, review_round,
                summary, result_markdown, created_at
         FROM task_submissions WHERE submission_id IN (SELECT value FROM json_each(?1))",
    )?;
    statement
        .query_map([ids_json], decode_base)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::Sqlite)
}

fn decode_base(row: &Row<'_>) -> rusqlite::Result<SubmissionBase> {
    Ok(SubmissionBase {
        submission_id: row.get(0)?,
        task_id: TaskId::new(row.get::<_, String>(1)?)
            .map_err(|error| conversion_failure(1, Type::Text, error))?,
        contract_id: TaskContractId::new(row.get::<_, String>(2)?)
            .map_err(|error| conversion_failure(2, Type::Text, error))?,
        executor_run_id: row.get(3)?,
        review_round: positive_u32(row, 4)?,
        summary: row.get(5)?,
        result_markdown: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn load_criteria(
    transaction: &Transaction<'_>,
    ids_json: &str,
) -> Result<HashMap<String, Vec<SubmissionCriterionEvidence>>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT submission_id, criterion_id, evidence_markdown
         FROM task_submission_criteria
         WHERE submission_id IN (SELECT value FROM json_each(?1))
         ORDER BY submission_id, criterion_id",
    )?;
    let rows = statement.query_map([ids_json], |row| {
        Ok((
            row.get::<_, String>(0)?,
            SubmissionCriterionEvidence {
                criterion_id: row.get(1)?,
                evidence_markdown: row.get(2)?,
            },
        ))
    })?;
    let mut grouped = HashMap::<String, Vec<SubmissionCriterionEvidence>>::new();
    for row in rows {
        let (id, criterion) = row?;
        let values = grouped.entry(id).or_default();
        if values.len() == 100 {
            return Err(invariant("submission criteria exceed the embedded limit"));
        }
        values.push(criterion);
    }
    Ok(grouped)
}

fn load_expected_criteria(
    transaction: &Transaction<'_>,
    bases: &[SubmissionBase],
) -> Result<HashMap<String, Vec<String>>, StoreError> {
    let mut contract_ids = bases
        .iter()
        .map(|base| base.contract_id.as_str().to_string())
        .collect::<Vec<_>>();
    contract_ids.sort();
    contract_ids.dedup();
    let ids_json = serde_json::to_string(&contract_ids)?;
    let mut grouped = contract_ids
        .into_iter()
        .map(|id| (id, Vec::new()))
        .collect::<HashMap<_, _>>();
    let mut statement = transaction.prepare(
        "SELECT contract_id, criterion_id FROM task_contract_criteria
         WHERE contract_id IN (SELECT value FROM json_each(?1))
         ORDER BY contract_id, criterion_id",
    )?;
    let rows = statement.query_map([ids_json], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (contract_id, criterion_id) = row?;
        let values = grouped
            .get_mut(&contract_id)
            .ok_or_else(|| invariant("criterion crosses submission contract batch"))?;
        if values.len() == 100 {
            return Err(invariant("contract criteria exceed the embedded limit"));
        }
        values.push(criterion_id);
    }
    Ok(grouped)
}

fn load_artifact_links(
    transaction: &Transaction<'_>,
    ids_json: &str,
) -> Result<Vec<ArtifactLink>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT submission_id, ordinal, artifact_id, artifact_version_id
         FROM task_submission_artifacts
         WHERE submission_id IN (SELECT value FROM json_each(?1))
         ORDER BY submission_id, ordinal",
    )?;
    let rows = statement.query_map([ids_json], |row| {
        Ok(ArtifactLink {
            submission_id: row.get(0)?,
            ordinal: positive_u32(row, 1)?,
            artifact_id: row.get(2)?,
            version_id: row.get(3)?,
        })
    })?;
    let links = rows.collect::<Result<Vec<_>, _>>()?;
    if links.windows(101).any(|window| {
        window
            .first()
            .zip(window.last())
            .is_some_and(|(first, last)| first.submission_id == last.submission_id)
    }) {
        return Err(invariant("submission artifacts exceed the embedded limit"));
    }
    Ok(links)
}

fn load_artifacts(
    transaction: &Transaction<'_>,
    links: &[ArtifactLink],
) -> Result<HashMap<String, noema_artifacts::ArtifactRecord>, StoreError> {
    use crate::artifacts::{ARTIFACT_SELECT, artifact_from_row, artifact_row};
    let ids_json = unique_ids(links.iter().map(|link| link.artifact_id.as_str()))?;
    let sql = format!(
        "SELECT {ARTIFACT_SELECT} FROM artifacts
         WHERE artifact_id IN (SELECT value FROM json_each(?1))"
    );
    let mut statement = transaction.prepare(&sql)?;
    let rows = statement.query_map([ids_json], artifact_row)?;
    let mut records = HashMap::new();
    for row in rows {
        let artifact = artifact_from_row(row?)?;
        records.insert(artifact.artifact_id.clone(), artifact);
    }
    Ok(records)
}

fn load_versions(
    transaction: &Transaction<'_>,
    links: &[ArtifactLink],
) -> Result<HashMap<String, noema_artifacts::ArtifactVersionRecord>, StoreError> {
    use crate::artifacts::{
        ARTIFACT_VERSION_SELECT, artifact_version_from_row, artifact_version_row,
    };
    let ids_json = unique_ids(links.iter().map(|link| link.version_id.as_str()))?;
    let sql = format!(
        "SELECT {ARTIFACT_VERSION_SELECT} FROM artifact_versions
         WHERE artifact_version_id IN (SELECT value FROM json_each(?1))"
    );
    let mut statement = transaction.prepare(&sql)?;
    let rows = statement.query_map([ids_json], artifact_version_row)?;
    let mut records = HashMap::new();
    for row in rows {
        let version = artifact_version_from_row(row?)?;
        records.insert(version.artifact_version_id.clone(), version);
    }
    Ok(records)
}

fn unique_ids<'a>(values: impl Iterator<Item = &'a str>) -> Result<String, StoreError> {
    let mut values = values.map(str::to_string).collect::<Vec<_>>();
    values.sort();
    values.dedup();
    serde_json::to_string(&values).map_err(StoreError::Json)
}

fn positive_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<u32> {
    let value = u32::try_from(row.get::<_, i64>(index)?)
        .map_err(|error| conversion_failure(index, Type::Integer, error))?;
    if value == 0 {
        Err(conversion_failure(
            index,
            Type::Integer,
            std::io::Error::new(std::io::ErrorKind::InvalidData, "expected positive integer"),
        ))
    } else {
        Ok(value)
    }
}

fn invariant(message: &'static str) -> StoreError {
    StoreError::InvariantViolation {
        message: message.to_string(),
    }
}
