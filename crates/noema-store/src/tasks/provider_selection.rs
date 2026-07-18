//! Transaction-local provider selection readers for durable task snapshots.

use noema_providers::{ProviderInstanceKey, ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{RunKind, TASK_REVIEWER_AGENT_ID, TaskComplexity};
use rusqlite::{OptionalExtension, Transaction};

use crate::{
    StoreError,
    provider_selections::{SelectionEligibility, validate_provider_selection_tx},
    sqlite::{parse_column, reasoning_column},
};

/// Load and validate the exact enabled pool selection inside its writer transaction.
pub(crate) fn pool_selection_tx(
    transaction: &Transaction<'_>,
    complexity: TaskComplexity,
    pool_entry_id: &str,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let row = transaction
        .query_row(
            r#"
            SELECT complexity, provider_kind, provider_account_id,
                   provider_instance_key, model_profile, reasoning_effort, enabled
            FROM task_model_pool_entries
            WHERE pool_entry_id = ?1
            LIMIT 1
            "#,
            [pool_entry_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, i64>(6)? != 0,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!("task model pool entry not found: {pool_entry_id}"),
        })?;
    let persisted_complexity =
        row.0
            .parse::<TaskComplexity>()
            .map_err(|error| StoreError::InvalidEnum {
                kind: "task_model_pool_complexity",
                value: error.to_string(),
            })?;
    if persisted_complexity != complexity {
        return Err(StoreError::InvariantViolation {
            message: format!(
                "task model pool entry {pool_entry_id} belongs to {persisted_complexity}, not {complexity}"
            ),
        });
    }
    if !row.6 {
        return Err(StoreError::InvariantViolation {
            message: format!("task model pool entry is disabled: {pool_entry_id}"),
        });
    }
    let selection_source = if noema_tasks::is_global_task_model_pool_setting_id(pool_entry_id) {
        "task_model_pool_setting"
    } else {
        "task_model_pool_override"
    };
    let selection = explicit_selection(row.1, row.2, row.3, row.4, row.5, selection_source)?;
    validate_provider_selection_tx(transaction, &selection, SelectionEligibility::Canonical)
}

/// Load the current built-in reviewer preference inside the task writer transaction.
pub(crate) fn reviewer_preference_tx(
    transaction: &Transaction<'_>,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let row = transaction
        .query_row(
            r#"
            SELECT provider_kind, provider_account_id, provider_instance_key,
                   model_profile, reasoning_effort
            FROM agent_runtime_preferences
            WHERE agent_id = ?1
            LIMIT 1
            "#,
            [TASK_REVIEWER_AGENT_ID],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::InvariantViolation {
            message: "task reviewer provider preference is missing".to_string(),
        })?;
    let selection = explicit_selection(row.0, row.1, row.2, row.3, row.4, "agent:task-reviewer")?;
    validate_provider_selection_tx(transaction, &selection, SelectionEligibility::Canonical)
}

/// Load one immutable task role selection and validate its future reference.
pub(crate) fn task_role_selection_tx(
    transaction: &Transaction<'_>,
    task_id: &str,
    run_kind: RunKind,
    eligibility: SelectionEligibility,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let sql = match run_kind {
        RunKind::Executor => {
            r#"
            SELECT executor_provider_kind, executor_provider_account_id,
                   executor_provider_instance_key, executor_selection_mode,
                   executor_model_profile, executor_reasoning_effort,
                   executor_selection_source
            FROM tasks WHERE task_id = ?1 LIMIT 1
            "#
        }
        RunKind::Reviewer => {
            r#"
            SELECT reviewer_provider_kind, reviewer_provider_account_id,
                   reviewer_provider_instance_key, reviewer_selection_mode,
                   reviewer_model_profile, reviewer_reasoning_effort,
                   reviewer_selection_source
            FROM tasks WHERE task_id = ?1 LIMIT 1
            "#
        }
    };
    let selection = transaction
        .query_row(sql, [task_id], selection_from_row)
        .optional()?
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!("task not found: {task_id}"),
        })?;
    validate_provider_selection_tx(transaction, &selection, eligibility)
}

/// Load one immutable run selection and validate a deliberately preserved reference.
pub(crate) fn preserved_run_selection_tx(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let selection = transaction
        .query_row(
            r#"
            SELECT provider_kind, provider_account_id, provider_instance_key,
                   selection_mode, model_profile, reasoning_effort, selection_source
            FROM agent_runs WHERE run_id = ?1 LIMIT 1
            "#,
            [run_id],
            selection_from_row,
        )
        .optional()?
        .ok_or_else(|| StoreError::InvariantViolation {
            message: format!("agent run not found: {run_id}"),
        })?;
    validate_provider_selection_tx(
        transaction,
        &selection,
        SelectionEligibility::PreservedFutureReference,
    )
}

fn explicit_selection(
    provider_kind: String,
    provider_account_id: String,
    provider_instance_key: String,
    model_profile: String,
    reasoning_effort: Option<String>,
    selection_source: &str,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let provider_instance_key =
        ProviderInstanceKey::new(provider_instance_key).map_err(|error| {
            StoreError::InvariantViolation {
                message: error.to_string(),
            }
        })?;
    let reasoning_effort = parse_reasoning(reasoning_effort.as_deref())?;
    let mut selection = ProviderSelectionSnapshot::explicit(
        provider_kind,
        provider_account_id,
        model_profile,
        reasoning_effort,
        Some(selection_source.to_string()),
    );
    selection.provider_instance_key = Some(provider_instance_key);
    Ok(selection)
}

fn selection_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderSelectionSnapshot> {
    let key = parse_column(row, 2)?;
    let selection_mode = parse_column(row, 3)?;
    let reasoning_effort = reasoning_column(row, 5)?;
    Ok(ProviderSelectionSnapshot {
        provider_kind: row.get(0)?,
        provider_account_id: row.get(1)?,
        provider_instance_key: Some(key),
        selection_mode,
        model_profile: row.get(4)?,
        reasoning_effort,
        selection_source: row.get(6)?,
    })
}

fn parse_reasoning(value: Option<&str>) -> Result<Option<ReasoningEffort>, StoreError> {
    value
        .map(|value| {
            ReasoningEffort::from_persistence_str(value).ok_or_else(|| StoreError::InvalidEnum {
                kind: "reasoning_effort",
                value: value.to_string(),
            })
        })
        .transpose()
}
