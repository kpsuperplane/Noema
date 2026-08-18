//! Transaction-local provider selection readers for durable task snapshots.

use noema_providers::{
    ModelPreferenceSelection, NoemaModelUseCase, ProviderInstanceKey, ProviderKind,
    ProviderSelectionSnapshot, ReasoningEffort,
};
use noema_tasks::{TASK_REVIEWER_AGENT_ID, TaskComplexity, model_use_case};
use rusqlite::{OptionalExtension, Transaction};

use crate::{
    StoreError,
    provider_selections::{SelectionEligibility, validate_provider_selection_tx},
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
                   provider_instance_key, selection_mode, model_profile, reasoning_effort,
                   fast_mode, enabled
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
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, i64>(7)? != 0,
                    row.get::<_, i64>(8)? != 0,
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
    if !row.8 {
        return Err(StoreError::InvariantViolation {
            message: format!("task model pool entry is disabled: {pool_entry_id}"),
        });
    }
    let selection_source = if noema_tasks::is_global_task_model_pool_setting_id(pool_entry_id) {
        "task_model_pool_setting"
    } else {
        "task_model_pool_override"
    };
    let preference = parse_preference(&row.4, row.5, row.6)?;
    let selection = effective_selection(
        row.1,
        row.2,
        row.3,
        preference,
        row.7,
        model_use_case(complexity),
        selection_source,
    )?;
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
                   selection_mode, model_profile, reasoning_effort, fast_mode
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
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, i64>(6)? != 0,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| StoreError::InvariantViolation {
            message: "task reviewer provider preference is missing".to_string(),
        })?;
    let preference = parse_preference(&row.3, row.4, row.5)?;
    let selection = effective_selection(
        row.0,
        row.1,
        row.2,
        preference,
        row.6,
        NoemaModelUseCase::TaskReviewer,
        "agent:task-reviewer",
    )?;
    validate_provider_selection_tx(transaction, &selection, SelectionEligibility::Canonical)
}

fn effective_selection(
    provider_kind: String,
    provider_account_id: String,
    provider_instance_key: String,
    preference: ModelPreferenceSelection,
    fast_mode: bool,
    use_case: NoemaModelUseCase,
    selection_source: &str,
) -> Result<ProviderSelectionSnapshot, StoreError> {
    let provider_instance_key =
        ProviderInstanceKey::new(provider_instance_key).map_err(|error| {
            StoreError::InvariantViolation {
                message: error.to_string(),
            }
        })?;
    let kind = provider_kind
        .parse::<ProviderKind>()
        .map_err(|_| StoreError::InvalidEnum {
            kind: "model_provider",
            value: provider_kind.clone(),
        })?;
    let (model_profile, reasoning_effort) =
        preference
            .resolve(kind, use_case)
            .ok_or_else(|| StoreError::InvariantViolation {
                message: "provider has no Noema recommendation for this use case".to_string(),
            })?;
    let mut selection = ProviderSelectionSnapshot::explicit(
        provider_kind,
        provider_account_id,
        model_profile,
        reasoning_effort,
        Some(selection_source.to_string()),
    );
    selection.provider_instance_key = Some(provider_instance_key);
    selection.fast_mode = fast_mode;
    Ok(selection)
}

fn parse_preference(
    mode: &str,
    model_profile: Option<String>,
    reasoning_effort: Option<String>,
) -> Result<ModelPreferenceSelection, StoreError> {
    let reasoning_effort = parse_reasoning(reasoning_effort.as_deref())?;
    ModelPreferenceSelection::from_persisted_parts(mode, model_profile, reasoning_effort)
        .ok_or_else(|| StoreError::InvalidEnum {
            kind: "model_preference_selection",
            value: mode.to_string(),
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
