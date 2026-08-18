use noema_providers::{ProviderKind, ProviderSelectionSnapshot};
use noema_tasks::{TaskModelPoolEntry, is_global_task_model_pool_setting_id, model_use_case};

use crate::sqlite::{model_preference_selection_column, parse_column};

pub(super) fn pool_entry_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskModelPoolEntry> {
    let pool_entry_id: String = row.get(0)?;
    let complexity = parse_column(row, 1)?;
    let provider_kind: String = row.get(3)?;
    let provider_account_id: String = row.get(4)?;
    let provider_instance_key = parse_column(row, 5)?;
    let preference = model_preference_selection_column(row, 6, 7, 8)?;
    let provider = provider_kind.parse::<ProviderKind>().map_err(|error| {
        crate::sqlite::conversion_failure(
            3,
            rusqlite::types::Type::Text,
            std::io::Error::new(std::io::ErrorKind::InvalidData, error),
        )
    })?;
    let (model_profile, reasoning_effort) = preference
        .resolve(provider, model_use_case(complexity))
        .ok_or_else(|| {
            crate::sqlite::conversion_failure(
                6,
                rusqlite::types::Type::Text,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "provider has no Noema recommendation",
                ),
            )
        })?;
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
    model.fast_mode = row.get::<_, i64>(9)? != 0;
    Ok(TaskModelPoolEntry {
        pool_entry_id: pool_entry_id.clone(),
        complexity,
        label: row.get(2)?,
        model,
        preference,
        enabled: row.get::<_, i64>(10)? != 0,
        sort_order: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}
