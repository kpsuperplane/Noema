use serde_json::Value;

use crate::{ProviderModelProfile, ReasoningEffort};

pub(super) fn profile_values_from_model_list(value: &Value) -> Vec<ProviderModelProfile> {
    value
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|model| model_is_visible(model))
        .filter_map(profile_value_from_model)
        .collect()
}

fn model_is_visible(model: &Value) -> bool {
    model
        .get("visibility")
        .and_then(Value::as_str)
        .is_some_and(|visibility| visibility == "list")
}

fn profile_value_from_model(model: &Value) -> Option<ProviderModelProfile> {
    let id = string_field(model, &["slug"])?.trim();
    if id.is_empty() {
        return None;
    }
    let label = string_field(model, &["display_name"]).unwrap_or(id);
    Some(ProviderModelProfile {
        id: id.to_string(),
        label: label.to_string(),
        reasoning_efforts: reasoning_efforts_from_model(model),
        default_reasoning_effort: string_field(
            model,
            &["default_reasoning_level", "default_reasoning_effort"],
        )
        .and_then(normalize_reasoning_effort),
        context_window_tokens: None,
    })
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn reasoning_efforts_from_model(model: &Value) -> Vec<ReasoningEffort> {
    let explicit = [
        "supported_reasoning_levels",
        "reasoning_levels",
        "reasoning_efforts",
    ]
    .iter()
    .find_map(|key| model.get(*key).and_then(Value::as_array))
    .map(|values| {
        values
            .iter()
            .filter_map(Value::as_str)
            .filter_map(normalize_reasoning_effort)
            .collect::<Vec<_>>()
    })
    .filter(|values| !values.is_empty());
    if let Some(explicit) = explicit {
        return explicit;
    }
    string_field(
        model,
        &["default_reasoning_level", "default_reasoning_effort"],
    )
    .and_then(normalize_reasoning_effort)
    .map_or_else(Vec::new, |_| {
        vec![
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh,
        ]
    })
}

fn normalize_reasoning_effort(value: &str) -> Option<ReasoningEffort> {
    ReasoningEffort::from_persistence_str(&value.trim().to_ascii_lowercase())
}
