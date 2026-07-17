use noema_providers::{ProviderSelectionSnapshot, ReasoningEffort};
use noema_tasks::{TaskComplexity, TaskModelPoolEntry, is_global_task_model_pool_setting_id};

use super::StoreError;

pub(super) fn pool_entry_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskModelPoolEntry> {
    let pool_entry_id: String = row.get(0)?;
    let complexity: String = row.get(1)?;
    let complexity = complexity.parse::<TaskComplexity>().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let provider_kind: String = row.get(3)?;
    let provider_account_id: String = row.get(4)?;
    let model_profile: String = row.get(5)?;
    let reasoning_effort: Option<String> = row.get(6)?;
    let reasoning_effort = parse_reasoning(reasoning_effort.as_deref()).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(TaskModelPoolEntry {
        pool_entry_id: pool_entry_id.clone(),
        complexity,
        label: row.get(2)?,
        model: ProviderSelectionSnapshot::explicit(
            provider_kind,
            provider_account_id,
            model_profile,
            reasoning_effort,
            Some(if is_global_task_model_pool_setting_id(&pool_entry_id) {
                "task_model_pool_setting".to_string()
            } else {
                "task_model_pool_override".to_string()
            }),
        ),
        enabled: row.get::<_, i64>(7)? != 0,
        sort_order: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
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
