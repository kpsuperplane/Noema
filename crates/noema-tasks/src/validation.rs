//! Shared normalization for task-domain input boundaries.

use crate::TaskDomainError;

pub(super) fn required(value: &str, field: &'static str) -> Result<String, TaskDomainError> {
    let value = value.trim();
    if value.is_empty() {
        Err(TaskDomainError::EmptyField(field))
    } else {
        Ok(value.to_string())
    }
}

pub(super) fn normalize_optional(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
