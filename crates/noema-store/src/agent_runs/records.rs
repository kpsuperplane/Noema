//! V3 agent-run row decoding used by the semantic Work writer.

use noema_tasks::{AgentRunItemRecord, AgentRunRecord, RunStatus, WorkDomainError};
use rusqlite::{Params, Row, Transaction, types::Type};

use super::WorkRunFence;
use crate::{
    StoreError,
    sqlite::{conversion_failure, json_column, parse_column},
    work_commands::helpers,
    work_events::WorkEventScope,
    work_reads::list_rows::load_runs,
};

pub(crate) fn load_run_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<Option<AgentRunRecord>, StoreError> {
    Ok(load_runs(transaction, &[run_id.to_string()])?.remove(run_id))
}

pub(crate) fn load_active_fenced_run_tx(
    transaction: &Transaction<'_>,
    fence: &WorkRunFence,
    expected_status: RunStatus,
) -> Result<AgentRunRecord, StoreError> {
    let run = load_run_tx(transaction, &fence.run_id)?
        .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
    if run.task_generation != fence.task_generation {
        return Err(StoreError::Work(WorkDomainError::StaleGeneration));
    }
    if run.contract_id != fence.contract_id
        || run.lease_token.as_deref() != Some(fence.lease_token.as_str())
    {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    if run.status != expected_status {
        return Err(StoreError::Work(WorkDomainError::InvalidTransition));
    }
    Ok(run)
}

pub(crate) fn event_scope(
    task: &helpers::TaskState,
    run_id: Option<&str>,
    actor_id: &str,
    causation_id: Option<&str>,
    correlation_id: &str,
) -> WorkEventScope {
    helpers::CommandEventContext {
        actor_id,
        causation_id,
        correlation_id,
    }
    .task_scope(task, run_id)
}

pub(crate) fn execute_fenced_update_tx(
    transaction: &Transaction<'_>,
    statement: &str,
    params: impl Params,
) -> Result<(), StoreError> {
    if transaction.execute(statement, params)? != 1 {
        return Err(StoreError::Work(WorkDomainError::RunFenced));
    }
    Ok(())
}

pub(crate) fn decode_run_item(
    row: &Row<'_>,
    max_payload_bytes: Option<usize>,
) -> rusqlite::Result<AgentRunItemRecord> {
    let payload_text = row.get::<_, String>(9)?;
    if max_payload_bytes.is_some_and(|limit| payload_text.len() > limit) {
        return Err(conversion_failure(
            9,
            Type::Text,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "run item payload exceeds bounded context",
            ),
        ));
    }
    Ok(AgentRunItemRecord {
        item_id: row.get(0)?,
        run_id: row.get(1)?,
        sequence_index: row.get(2)?,
        round_index: row.get(3)?,
        kind: parse_column(row, 4)?,
        status: parse_column(row, 5)?,
        correlation_id: row.get(6)?,
        parent_item_id: row.get(7)?,
        content_text: row.get(8)?,
        payload: json_column(row, 9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}
