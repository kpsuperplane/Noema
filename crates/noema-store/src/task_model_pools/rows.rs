use noema_providers::ProviderSelectionSnapshot;
use noema_tasks::{TaskModelPoolEntry, is_global_task_model_pool_setting_id};

use crate::sqlite::{parse_column, reasoning_column};

pub(super) fn pool_entry_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskModelPoolEntry> {
    let pool_entry_id: String = row.get(0)?;
    let complexity = parse_column(row, 1)?;
    let provider_kind: String = row.get(3)?;
    let provider_account_id: String = row.get(4)?;
    let provider_instance_key = parse_column(row, 5)?;
    let model_profile: String = row.get(6)?;
    let reasoning_effort = reasoning_column(row, 7)?;
    let mut model = ProviderSelectionSnapshot::explicit(
        provider_kind,
        provider_account_id,
        model_profile,
        reasoning_effort,
        Some(if is_global_task_model_pool_setting_id(&pool_entry_id) {
            "task_model_pool_setting".to_string()
        } else {
            "task_model_pool_override".to_string()
        }),
    );
    model.provider_instance_key = Some(provider_instance_key);
    Ok(TaskModelPoolEntry {
        pool_entry_id: pool_entry_id.clone(),
        complexity,
        label: row.get(2)?,
        model,
        is_override: row.get(8)?,
        enabled: row.get::<_, i64>(9)? != 0,
        sort_order: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}
