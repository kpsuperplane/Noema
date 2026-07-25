use serde::Deserialize;
use thiserror::Error;

use crate::{
    McpToolHint, McpToolHintSource, McpToolPolicyRecord, McpToolPolicyStatus, McpToolRecord,
};

/// Strict model output for only the missing MCP behavior hints.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolHintCompletion {
    /// Inferred read-only value when that hint was missing.
    pub read_only: Option<bool>,
    /// Inferred idempotency value when that hint was missing.
    pub idempotent: Option<bool>,
    /// Inferred destructive value when that hint was missing.
    pub destructive: Option<bool>,
    /// Inferred open-world value when that hint was missing.
    pub open_world: Option<bool>,
}

#[derive(Debug, Error)]
pub(crate) enum McpToolClassificationError {
    #[error("invalid tool-hint JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("tool-hint response did not match the missing fields")]
    FieldMismatch,
}

/// Build a bounded metadata-only prompt for one incomplete tool.
#[must_use]
pub(crate) fn build_classification_prompt(
    tool: &McpToolRecord,
    policy: &McpToolPolicyRecord,
) -> String {
    let missing = missing_fields(policy).join(",");
    let description = tool.description.as_deref().unwrap_or("-");
    let input = schema_fields(&tool.input_schema);
    let output = tool
        .output_schema
        .as_ref()
        .map_or_else(|| "-".to_string(), schema_fields);
    format!(
        "Classify only these missing MCP tool behavior hints: {missing}.\nReturn one strict JSON object using only the corresponding camelCase keys and boolean values.\nreadOnly=no environment mutation; idempotent=repeating identical arguments adds no effect; destructive=may overwrite/delete; openWorld=may interact with external entities.\ntool={}\ndescription={}\ninput_fields={}\noutput_fields={}",
        sanitize(&tool.name, 128),
        sanitize(description, 256),
        input,
        output,
    )
}

pub(crate) fn parse_classification_response(
    text: &str,
    policy: &McpToolPolicyRecord,
) -> Result<McpToolHintCompletion, McpToolClassificationError> {
    let completion: McpToolHintCompletion = serde_json::from_str(text.trim())?;
    let expected = [
        policy.read_only.value.is_none(),
        policy.idempotent.value.is_none(),
        policy.destructive.value.is_none(),
        policy.open_world.value.is_none(),
    ];
    let actual = [
        completion.read_only.is_some(),
        completion.idempotent.is_some(),
        completion.destructive.is_some(),
        completion.open_world.is_some(),
    ];
    if expected != actual {
        return Err(McpToolClassificationError::FieldMismatch);
    }
    Ok(completion)
}

/// Merge a validated completion without replacing annotation-owned fields.
#[must_use]
pub(crate) fn apply_completion(
    mut policy: McpToolPolicyRecord,
    completion: McpToolHintCompletion,
) -> McpToolPolicyRecord {
    fill(
        &mut policy.read_only,
        completion.read_only,
        false,
        McpToolHintSource::Model,
    );
    fill(
        &mut policy.idempotent,
        completion.idempotent,
        false,
        McpToolHintSource::Model,
    );
    fill(
        &mut policy.destructive,
        completion.destructive,
        true,
        McpToolHintSource::Model,
    );
    fill(
        &mut policy.open_world,
        completion.open_world,
        true,
        McpToolHintSource::Model,
    );
    policy.status = McpToolPolicyStatus::Ready;
    policy
}

/// Fill missing hints with MCP's pessimistic defaults after model failure.
#[must_use]
pub(crate) fn apply_safe_defaults(mut policy: McpToolPolicyRecord) -> McpToolPolicyRecord {
    fill(
        &mut policy.read_only,
        None,
        false,
        McpToolHintSource::SafeDefault,
    );
    fill(
        &mut policy.idempotent,
        None,
        false,
        McpToolHintSource::SafeDefault,
    );
    fill(
        &mut policy.destructive,
        None,
        true,
        McpToolHintSource::SafeDefault,
    );
    fill(
        &mut policy.open_world,
        None,
        true,
        McpToolHintSource::SafeDefault,
    );
    policy.status = McpToolPolicyStatus::Defaulted;
    policy
}

fn fill(hint: &mut McpToolHint, inferred: Option<bool>, default: bool, source: McpToolHintSource) {
    if hint.value.is_none() {
        hint.value = Some(inferred.unwrap_or(default));
        hint.source = Some(source);
    }
}

fn missing_fields(policy: &McpToolPolicyRecord) -> Vec<&'static str> {
    [
        ("readOnly", policy.read_only.value),
        ("idempotent", policy.idempotent.value),
        ("destructive", policy.destructive.value),
        ("openWorld", policy.open_world.value),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.is_none().then_some(name))
    .collect()
}

fn schema_fields(schema: &serde_json::Value) -> String {
    schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|properties| {
            properties
                .keys()
                .map(|key| sanitize(key, 64))
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|fields| !fields.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

fn sanitize(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(max_chars)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn completion_can_fill_only_missing_hints() {
        let policy = McpToolPolicyRecord {
            mcp_tool_id: "tool".into(),
            read_only: McpToolHint {
                value: Some(true),
                source: Some(McpToolHintSource::Annotation),
            },
            idempotent: McpToolHint {
                value: None,
                source: None,
            },
            destructive: McpToolHint {
                value: Some(false),
                source: Some(McpToolHintSource::Annotation),
            },
            open_world: McpToolHint {
                value: None,
                source: None,
            },
            status: McpToolPolicyStatus::Pending,
            policy_revision: 1,
            metadata_fingerprint: "fingerprint".into(),
        };
        let completion = parse_classification_response(
            &json!({"idempotent": true, "openWorld": false}).to_string(),
            &policy,
        )
        .expect("completion");
        let merged = apply_completion(policy, completion);
        assert_eq!(merged.read_only.source, Some(McpToolHintSource::Annotation));
        assert_eq!(merged.idempotent.source, Some(McpToolHintSource::Model));
        assert!(merged.is_callable());

        let defaulted = apply_safe_defaults(McpToolPolicyRecord {
            idempotent: McpToolHint {
                value: None,
                source: None,
            },
            open_world: McpToolHint {
                value: None,
                source: None,
            },
            status: McpToolPolicyStatus::Pending,
            ..merged
        });
        assert_eq!(defaulted.status, McpToolPolicyStatus::Defaulted);
        assert_eq!(defaulted.idempotent.value, Some(false));
        assert_eq!(
            defaulted.idempotent.source,
            Some(McpToolHintSource::SafeDefault)
        );
        assert_eq!(defaulted.open_world.value, Some(true));
    }
}
