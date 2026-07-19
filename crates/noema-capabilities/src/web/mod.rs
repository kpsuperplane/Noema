//! Stable Noema-visible web capability contracts.

use serde_json::Value;

pub mod fetch;
pub mod search;
pub mod url_policy;

pub(crate) fn nested_arguments(payload: &Value) -> Result<Value, &'static str> {
    let valid = payload
        .as_object()
        .is_some_and(|o| o.keys().all(|k| k == "arguments"));
    match payload.get("arguments") {
        Some(arguments) if valid => Ok(arguments.clone()),
        Some(_) => Err("nested arguments payload cannot include outer fields"),
        _ => Ok(payload.clone()),
    }
}

pub(crate) fn normalize_reason(
    reason: Option<String>,
    max_chars: usize,
) -> Result<Option<String>, usize> {
    let reason = reason
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty());
    if reason
        .as_deref()
        .is_some_and(|value| value.chars().count() > max_chars)
    {
        Err(max_chars)
    } else {
        Ok(reason)
    }
}
