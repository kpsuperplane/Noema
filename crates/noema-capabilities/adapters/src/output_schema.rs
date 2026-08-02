//! Closed validation for transformed JSON output.

use crate::{OutputSchema, OutputType};
use serde_json::Value;
use std::collections::BTreeSet;

const MAX_SCHEMA_DEPTH: usize = 16;
const MAX_SCHEMA_NODES: usize = 512;
pub(crate) const MAX_MODEL_RESULT_BYTES: usize = 32 * 1024;

pub(crate) fn validate(schema: &OutputSchema) -> bool {
    fn visit(schema: &OutputSchema, depth: usize, nodes: &mut usize) -> bool {
        *nodes += 1;
        if depth > MAX_SCHEMA_DEPTH || *nodes > MAX_SCHEMA_NODES {
            return false;
        }
        let object_fields = !schema.properties.is_empty()
            || !schema.required.is_empty()
            || schema.additional_properties.is_some();
        match schema.value_type {
            OutputType::Object => {
                if schema.additional_properties != Some(false)
                    || schema.items.is_some()
                    || schema.max_bytes.is_some()
                    || schema.max_items.is_some()
                    || schema.properties.len() > MAX_SCHEMA_NODES
                {
                    return false;
                }
                let mut required = BTreeSet::new();
                schema.required.iter().all(|name| {
                    required.insert(name)
                        && schema.properties.contains_key(name)
                        && valid_name(name)
                }) && schema
                    .properties
                    .iter()
                    .all(|(name, child)| valid_name(name) && visit(child, depth + 1, nodes))
            }
            OutputType::Array => {
                !object_fields
                    && schema.max_bytes.is_none()
                    && schema.max_items.is_some()
                    && schema
                        .items
                        .as_deref()
                        .is_some_and(|child| visit(child, depth + 1, nodes))
            }
            OutputType::String => {
                !object_fields
                    && schema.items.is_none()
                    && schema.max_bytes.is_some()
                    && schema.max_items.is_none()
            }
            _ => {
                !object_fields
                    && schema.items.is_none()
                    && schema.max_bytes.is_none()
                    && schema.max_items.is_none()
            }
        }
    }
    visit(schema, 0, &mut 0)
}

#[allow(dead_code)]
pub(crate) fn matches(schema: &OutputSchema, value: &Value) -> bool {
    match (schema.value_type, value) {
        (OutputType::Object, Value::Object(values)) => {
            values
                .keys()
                .all(|name| schema.properties.contains_key(name))
                && schema.required.iter().all(|name| values.contains_key(name))
                && values.iter().all(|(name, value)| {
                    schema
                        .properties
                        .get(name)
                        .is_some_and(|child| matches(child, value))
                })
        }
        (OutputType::Array, Value::Array(values)) => schema.max_items.is_some_and(|max| {
            values.len() <= max
                && schema
                    .items
                    .as_deref()
                    .is_some_and(|child| values.iter().all(|value| matches(child, value)))
        }),
        (OutputType::String, Value::String(value)) => {
            schema.max_bytes.is_some_and(|max| value.len() <= max)
        }
        (OutputType::Boolean, Value::Bool(_)) => true,
        (OutputType::Integer, Value::Number(number)) => number.is_i64() || number.is_u64(),
        (OutputType::Number, Value::Number(_)) | (OutputType::Null, Value::Null) => true,
        _ => false,
    }
}

/// Conservatively bound the largest compact JSON serialization admitted by a schema.
pub(crate) fn maximum_serialized_bytes(schema: &OutputSchema) -> Option<usize> {
    fn add(left: usize, right: usize) -> Option<usize> {
        left.checked_add(right)
    }
    match schema.value_type {
        OutputType::Object => {
            let mut total = 2usize;
            for (index, (name, child)) in schema.properties.iter().enumerate() {
                if index > 0 {
                    total = add(total, 1)?;
                }
                // Every input byte may require a six-byte JSON escape.
                total = add(total, name.len().checked_mul(6)?.checked_add(3)?)?;
                total = add(total, maximum_serialized_bytes(child)?)?;
            }
            Some(total)
        }
        OutputType::Array => {
            let items = schema.max_items?;
            let item = maximum_serialized_bytes(schema.items.as_deref()?)?;
            let contents = item.checked_mul(items)?;
            let separators = items.saturating_sub(1);
            add(add(2, contents)?, separators)
        }
        OutputType::String => schema.max_bytes?.checked_mul(6)?.checked_add(2),
        OutputType::Integer => Some(20),
        OutputType::Number => Some(24),
        OutputType::Boolean => Some(5),
        OutputType::Null => Some(4),
    }
}

fn valid_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
