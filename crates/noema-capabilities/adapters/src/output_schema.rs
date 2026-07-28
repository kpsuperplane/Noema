//! Closed validation for transformed JSON output.

use crate::{OutputSchema, OutputType};
use serde_json::Value;
use std::collections::BTreeSet;

const MAX_SCHEMA_DEPTH: usize = 16;
const MAX_SCHEMA_NODES: usize = 512;

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
                    && schema
                        .items
                        .as_deref()
                        .is_some_and(|child| visit(child, depth + 1, nodes))
            }
            _ => !object_fields && schema.items.is_none(),
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
        (OutputType::Array, Value::Array(values)) => schema
            .items
            .as_deref()
            .is_some_and(|child| values.iter().all(|value| matches(child, value))),
        (OutputType::String, Value::String(_)) | (OutputType::Boolean, Value::Bool(_)) => true,
        (OutputType::Integer, Value::Number(number)) => number.is_i64() || number.is_u64(),
        (OutputType::Number, Value::Number(_)) | (OutputType::Null, Value::Null) => true,
        _ => false,
    }
}

fn valid_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
