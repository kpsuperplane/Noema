//! Exact, structured decimal calculations for Task execution and review.

use std::{collections::BTreeMap, str::FromStr};

use bigdecimal::{BigDecimal, RoundingMode};
use noema_capabilities::{ToolContractError, ToolSpec};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) const CALCULATE_TOOL: &str = "calculation.evaluate";
const MAX_INPUTS: usize = 200;
const MAX_STEPS: usize = 100;
const MAX_OPERANDS: usize = 200;
const MAX_SCALE: i64 = 12;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CalculationArguments {
    inputs: BTreeMap<String, String>,
    steps: Vec<CalculationStep>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CalculationStep {
    id: String,
    operation: CalculationOperation,
    operands: Vec<CalculationOperand>,
    #[serde(default)]
    scale: Option<i64>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CalculationOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Sum,
    Average,
    Minimum,
    Maximum,
    Round,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CalculationOperand {
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    step: Option<String>,
    #[serde(default)]
    literal: Option<String>,
}

pub(super) fn is_calculation_tool(name: &str) -> bool {
    name == CALCULATE_TOOL
}

pub(super) fn calculate_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        CALCULATE_TOOL,
        "Evaluate ordered decimal calculations exactly from named saved inputs. Use the returned formulas and values as the reproducible authority for every reported total.",
        json!({
            "type": "object",
            "properties": {
                "inputs": {
                    "type": "object", "maxProperties": MAX_INPUTS,
                    "additionalProperties": {
                        "type": "string", "pattern": "^-?[0-9]+(?:\\.[0-9]+)?$", "maxLength": 80
                    }
                },
                "steps": {
                    "type": "array", "minItems": 1, "maxItems": MAX_STEPS,
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_-]{0,63}$"},
                            "operation": {"type": "string", "enum": [
                                "add", "subtract", "multiply", "divide", "sum",
                                "average", "minimum", "maximum", "round"
                            ]},
                            "operands": {
                                "type": "array", "minItems": 1, "maxItems": MAX_OPERANDS,
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "input": {"type": "string", "maxLength": 64},
                                        "step": {"type": "string", "maxLength": 64},
                                        "literal": {"type": "string", "pattern": "^-?[0-9]+(?:\\.[0-9]+)?$", "maxLength": 80}
                                    },
                                    "minProperties": 1, "maxProperties": 1,
                                    "additionalProperties": false
                                }
                            },
                            "scale": {"type": "integer", "minimum": 0, "maximum": MAX_SCALE}
                        },
                        "required": ["id", "operation", "operands"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["inputs", "steps"],
            "additionalProperties": false
        }),
    )
}

pub(super) fn execute_calculation(payload: &Value) -> Result<Value, String> {
    let arguments: CalculationArguments = serde_json::from_value(payload.clone())
        .map_err(|error| format!("invalid calculation arguments: {error}"))?;
    if arguments.inputs.len() > MAX_INPUTS || arguments.steps.len() > MAX_STEPS {
        return Err("calculation exceeds its bounded input or step limit".to_string());
    }
    let inputs = parse_inputs(arguments.inputs)?;
    let mut results = BTreeMap::new();
    let mut output = Vec::with_capacity(arguments.steps.len());
    for step in arguments.steps {
        validate_identifier(&step.id)?;
        if results.contains_key(&step.id) || inputs.contains_key(&step.id) {
            return Err(format!(
                "calculation identifier '{}' is not unique",
                step.id
            ));
        }
        if step.operands.len() > MAX_OPERANDS {
            return Err(format!(
                "calculation step '{}' has too many operands",
                step.id
            ));
        }
        let operands = step
            .operands
            .iter()
            .map(|operand| resolve_operand(operand, &inputs, &results))
            .collect::<Result<Vec<_>, _>>()?;
        let value = evaluate_step(step.operation, &operands, step.scale)?;
        output.push(json!({
            "id": step.id,
            "value": render_decimal(&value, step.scale),
            "operation": operation_name(step.operation),
            "operands": step.operands.iter().map(operand_label).collect::<Result<Vec<_>, _>>()?,
            "scale": step.scale
        }));
        results.insert(step.id, value);
    }
    Ok(json!({
        "inputs": inputs.into_iter().map(|(id, value)| (id, render_decimal(&value, None))).collect::<BTreeMap<_, _>>(),
        "steps": output
    }))
}

fn parse_inputs(values: BTreeMap<String, String>) -> Result<BTreeMap<String, BigDecimal>, String> {
    values
        .into_iter()
        .map(|(id, value)| {
            validate_identifier(&id)?;
            parse_decimal(&value).map(|value| (id, value))
        })
        .collect()
}

fn validate_identifier(value: &str) -> Result<(), String> {
    let valid = value.len() <= 64
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(format!("calculation identifier '{value}' is invalid"))
    }
}

fn parse_decimal(value: &str) -> Result<BigDecimal, String> {
    BigDecimal::from_str(value).map_err(|_| format!("decimal value '{value}' is invalid"))
}

fn resolve_operand(
    operand: &CalculationOperand,
    inputs: &BTreeMap<String, BigDecimal>,
    results: &BTreeMap<String, BigDecimal>,
) -> Result<BigDecimal, String> {
    match (&operand.input, &operand.step, &operand.literal) {
        (Some(id), None, None) => inputs
            .get(id)
            .cloned()
            .ok_or_else(|| format!("calculation input '{id}' is unavailable")),
        (None, Some(id), None) => results
            .get(id)
            .cloned()
            .ok_or_else(|| format!("calculation step '{id}' is unavailable")),
        (None, None, Some(value)) => parse_decimal(value),
        _ => Err(
            "each calculation operand must select one input, prior step, or literal".to_string(),
        ),
    }
}

fn operand_label(operand: &CalculationOperand) -> Result<Value, String> {
    match (&operand.input, &operand.step, &operand.literal) {
        (Some(value), None, None) => Ok(json!({"input": value})),
        (None, Some(value), None) => Ok(json!({"step": value})),
        (None, None, Some(value)) => Ok(json!({"literal": value})),
        _ => Err(
            "each calculation operand must select one input, prior step, or literal".to_string(),
        ),
    }
}

fn evaluate_step(
    operation: CalculationOperation,
    operands: &[BigDecimal],
    scale: Option<i64>,
) -> Result<BigDecimal, String> {
    require_operands(operation, operands.len())?;
    let value = match operation {
        CalculationOperation::Add | CalculationOperation::Sum => operands.iter().cloned().sum(),
        CalculationOperation::Subtract => &operands[0] - &operands[1],
        CalculationOperation::Multiply => operands
            .iter()
            .cloned()
            .fold(BigDecimal::from(1), |product, value| product * value),
        CalculationOperation::Divide => {
            if operands[1] == 0 {
                return Err("divide cannot use a zero divisor".to_string());
            }
            (&operands[0] / &operands[1])
                .with_scale_round(required_scale(scale, operation)?, RoundingMode::HalfEven)
        }
        CalculationOperation::Average => (operands.iter().cloned().sum::<BigDecimal>()
            / BigDecimal::from(
                u64::try_from(operands.len()).expect("bounded operand count fits u64"),
            ))
        .with_scale_round(required_scale(scale, operation)?, RoundingMode::HalfEven),
        CalculationOperation::Minimum => operands.iter().min().cloned().expect("not empty"),
        CalculationOperation::Maximum => operands.iter().max().cloned().expect("not empty"),
        CalculationOperation::Round => {
            operands[0].with_scale_round(required_scale(scale, operation)?, RoundingMode::HalfEven)
        }
    };
    match scale {
        Some(scale)
            if !matches!(
                operation,
                CalculationOperation::Divide
                    | CalculationOperation::Average
                    | CalculationOperation::Round
            ) =>
        {
            Ok(value.with_scale_round(scale, RoundingMode::HalfEven))
        }
        _ => Ok(value),
    }
}

fn require_operands(operation: CalculationOperation, count: usize) -> Result<(), String> {
    let valid = match operation {
        CalculationOperation::Add | CalculationOperation::Multiply => count >= 2,
        CalculationOperation::Subtract | CalculationOperation::Divide => count == 2,
        CalculationOperation::Sum
        | CalculationOperation::Average
        | CalculationOperation::Minimum
        | CalculationOperation::Maximum => count >= 1,
        CalculationOperation::Round => count == 1,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{} received an invalid operand count",
            operation_name(operation)
        ))
    }
}

fn required_scale(scale: Option<i64>, operation: CalculationOperation) -> Result<i64, String> {
    match scale {
        Some(scale) if (0..=MAX_SCALE).contains(&scale) => Ok(scale),
        Some(_) => Err("calculation scale must be from 0 through 12".to_string()),
        None => Err(format!(
            "{} requires an explicit scale",
            operation_name(operation)
        )),
    }
}

fn render_decimal(value: &BigDecimal, scale: Option<i64>) -> String {
    scale.map_or_else(|| value.normalized().to_string(), |_| value.to_string())
}

const fn operation_name(operation: CalculationOperation) -> &'static str {
    match operation {
        CalculationOperation::Add => "add",
        CalculationOperation::Subtract => "subtract",
        CalculationOperation::Multiply => "multiply",
        CalculationOperation::Divide => "divide",
        CalculationOperation::Sum => "sum",
        CalculationOperation::Average => "average",
        CalculationOperation::Minimum => "minimum",
        CalculationOperation::Maximum => "maximum",
        CalculationOperation::Round => "round",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(payload: Value) -> Value {
        execute_calculation(&payload).expect("calculation succeeds")
    }

    #[test]
    fn exact_decimals_and_prior_steps_reproduce_totals() {
        let result = run(json!({
            "inputs": {"subtotal": "0.10", "tax_rate": "0.20"},
            "steps": [
                {"id": "tax", "operation": "multiply", "operands": [{"input": "subtotal"}, {"input": "tax_rate"}]},
                {"id": "total", "operation": "add", "operands": [{"input": "subtotal"}, {"step": "tax"}]}
            ]
        }));
        assert_eq!(result["steps"][0]["value"], "0.02");
        assert_eq!(result["steps"][1]["value"], "0.12");
    }

    #[test]
    fn division_and_average_use_explicit_half_even_scale() {
        let result = run(json!({
            "inputs": {},
            "steps": [
                {"id": "third", "operation": "divide", "operands": [{"literal": "10"}, {"literal": "3"}], "scale": 2},
                {"id": "mean", "operation": "average", "operands": [{"literal": "1"}, {"literal": "2"}, {"literal": "2"}], "scale": 2}
            ]
        }));
        assert_eq!(result["steps"][0]["value"], "3.33");
        assert_eq!(result["steps"][1]["value"], "1.67");
    }

    #[test]
    fn round_uses_half_even() {
        let result = run(json!({
            "inputs": {},
            "steps": [{"id": "rounded", "operation": "round", "operands": [{"literal": "2.345"}], "scale": 2}]
        }));
        assert_eq!(result["steps"][0]["value"], "2.34");
    }

    #[test]
    fn future_step_reference_is_rejected() {
        let error = execute_calculation(&json!({
            "inputs": {},
            "steps": [{"id": "first", "operation": "sum", "operands": [{"step": "later"}]}]
        }))
        .expect_err("future step must fail");
        assert!(error.contains("step 'later' is unavailable"));
    }

    #[test]
    fn zero_division_is_rejected() {
        let error = execute_calculation(&json!({
            "inputs": {},
            "steps": [{"id": "bad", "operation": "divide", "operands": [{"literal": "1"}, {"literal": "0"}], "scale": 2}]
        }))
        .expect_err("zero division must fail");
        assert_eq!(error, "divide cannot use a zero divisor");
    }

    #[test]
    fn one_operand_object_cannot_select_two_sources() {
        let error = execute_calculation(&json!({
            "inputs": {"value": "1"},
            "steps": [{"id": "bad", "operation": "sum", "operands": [{"input": "value", "literal": "1"}]}]
        }))
        .expect_err("ambiguous operand must fail");
        assert!(error.contains("one input, prior step, or literal"));
    }
}
