use serde_json::Value;

use super::MemoryPersistenceError;

pub(super) async fn allocate_id(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    prefix: &str,
) -> Result<String, MemoryPersistenceError> {
    let hex: String = sqlx::query_scalar("SELECT encode(gen_random_bytes(16), 'hex')")
        .fetch_one(executor)
        .await
        .map_err(MemoryPersistenceError::Database)?;
    Ok(format!("{prefix}_{hex}"))
}

pub(super) fn json_value(value: Value) -> Value {
    value
}
