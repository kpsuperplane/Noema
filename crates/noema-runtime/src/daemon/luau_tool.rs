//! Bounded Luau execution for agent reasoning.

use noema_capabilities::{ToolContractError, ToolSpec};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) const LUAU_RUN_TOOL: &str = "code.run_luau";
const MAX_SOURCE_CHARS: usize = 50_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LuauArguments {
    source: String,
    #[serde(default)]
    input: Value,
}

pub(super) fn is_luau_run_tool(name: &str) -> bool {
    name == LUAU_RUN_TOOL
}

pub(super) fn luau_run_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        LUAU_RUN_TOOL,
        "Run bounded sandboxed Luau over a read-only JSON object and return one JSON value. The sandbox has no files, network, processes, clock, modules, or random state. Read input from the global `input`. Use integer minor units for exact money. Use json.object() or json.array() for empty tables.",
        json!({
            "type": "object",
            "properties": {
                "source": {"type": "string", "minLength": 1, "maxLength": MAX_SOURCE_CHARS},
                "input": {"type": "object", "additionalProperties": true}
            },
            "required": ["source"],
            "additionalProperties": false
        }),
    )
}

pub(super) fn execute_luau(payload: &Value) -> Result<Value, String> {
    let arguments: LuauArguments = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid Luau arguments: {error}"))?;
    if arguments.source.chars().count() > MAX_SOURCE_CHARS {
        return Err("Luau source exceeds its character limit".to_string());
    }
    noema_capability_adapters::run_sandboxed_luau(&arguments.source, &arguments.input)
        .map(|value| json!({"value": value}))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_general_json_computation() {
        assert_eq!(
            execute_luau(&json!({
                "source": "local total = 0 for _, value in input.values do total += value end return { total = total, count = #input.values }",
                "input": {"values": [125, 250, 375]}
            }))
            .expect("Luau result"),
            json!({"value": {"total": 750, "count": 3}})
        );
    }

    #[test]
    fn rejects_unbounded_or_non_json_results() {
        assert!(execute_luau(&json!({"source": "while true do end"})).is_err());
        assert!(execute_luau(&json!({"source": "return function() end"})).is_err());
    }

    #[test]
    fn input_schema_declares_an_open_json_object() {
        let spec = luau_run_tool_spec().expect("Luau tool");
        assert_eq!(
            spec.input_schema.as_value()["properties"]["input"],
            json!({"type": "object", "additionalProperties": true}),
        );
    }
}
