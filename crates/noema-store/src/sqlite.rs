use std::{error::Error, str::FromStr};

use noema_providers::{ModelPreferenceSelection, ReasoningEffort};
use rusqlite::{Row, types::Type};
use serde::{Serialize, de::DeserializeOwned};

use super::StoreError;

pub(super) fn serialize_json<T: Serialize>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(StoreError::Json)
}

pub(super) fn deserialize_json<T: DeserializeOwned>(value: String) -> Result<T, StoreError> {
    serde_json::from_str(&value).map_err(StoreError::Json)
}

pub(super) fn parse_column<T>(row: &Row<'_>, index: usize) -> rusqlite::Result<T>
where
    T: FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    row.get::<_, String>(index)?
        .parse()
        .map_err(|error| conversion_failure(index, Type::Text, error))
}

pub(super) fn reasoning_column(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<ReasoningEffort>> {
    row.get::<_, Option<String>>(index)?
        .map(|value| {
            ReasoningEffort::from_persistence_str(&value).ok_or_else(|| {
                conversion_failure(
                    index,
                    Type::Text,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid reasoning effort",
                    ),
                )
            })
        })
        .transpose()
}

pub(super) fn model_preference_selection_column(
    row: &Row<'_>,
    mode_index: usize,
    model_index: usize,
    reasoning_index: usize,
) -> rusqlite::Result<ModelPreferenceSelection> {
    let mode = row.get::<_, String>(mode_index)?;
    let model_profile = row.get(model_index)?;
    let reasoning_effort = reasoning_column(row, reasoning_index)?;
    ModelPreferenceSelection::from_persisted_parts(&mode, model_profile, reasoning_effort)
        .ok_or_else(|| {
            conversion_failure(
                mode_index,
                Type::Text,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid model preference selection",
                ),
            )
        })
}

pub(super) fn json_column<T: DeserializeOwned>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    serde_json::from_str(&row.get::<_, String>(index)?)
        .map_err(|error| conversion_failure(index, Type::Text, error))
}

pub(super) fn conversion_failure(
    index: usize,
    source_type: Type,
    error: impl Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, source_type, Box::new(error))
}

pub(super) fn now_timestamp_sql() -> &'static str {
    "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
}
