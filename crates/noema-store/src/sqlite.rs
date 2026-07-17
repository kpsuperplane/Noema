#![allow(dead_code)]

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::StoreError;

pub(super) fn json_to_string(value: &Value) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(StoreError::Json)
}

pub(super) fn serialize_json<T: Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(StoreError::Json)
}

pub(super) fn json_from_string(value: String) -> Result<Value, StoreError> {
    serde_json::from_str(&value).map_err(StoreError::Json)
}

pub(super) fn deserialize_json<T: DeserializeOwned>(value: String) -> Result<T, StoreError> {
    serde_json::from_str(&value).map_err(StoreError::Json)
}

pub(super) fn optional_row<T, F>(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
    mapper: F,
) -> Result<Option<T>, StoreError>
where
    F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
{
    conn.query_row(sql, params, mapper)
        .optional()
        .map_err(StoreError::Sqlite)
}

pub(super) fn now_timestamp_sql() -> &'static str {
    "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
}
