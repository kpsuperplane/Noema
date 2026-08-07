use std::time::{Duration, Instant};

use noema_providers::{GenerateStreamEvent, ProviderHandle, ReasoningEffort};

use super::{
    cases::evaluation_cases_for_roles,
    grade::grade_response,
    types::{
        EvalCase, RuntimeEvalCaseDescriptor, RuntimeEvalCaseResult, RuntimeEvalRole,
        RuntimeEvalToolCall,
    },
};

/// Run the deterministic runtime-sensitive scenarios against one ready provider.
///
/// # Errors
///
/// Returns an error when the production-derived scenario fixtures cannot be
/// constructed. Per-scenario provider and grading failures are retained in the
/// returned results.
pub async fn run_runtime_suite(
    provider: &ProviderHandle,
    model_id: &str,
) -> Result<Vec<RuntimeEvalCaseResult>, String> {
    run_runtime_suite_for_roles(provider, model_id, RuntimeEvalRole::ALL, None).await
}

/// Run only cases assigned to the requested model settings.
///
/// # Errors
/// Returns an error when the production-derived fixtures cannot be constructed.
pub async fn run_runtime_suite_for_roles(
    provider: &ProviderHandle,
    model_id: &str,
    roles: &[RuntimeEvalRole],
    reasoning_effort: Option<ReasoningEffort>,
) -> Result<Vec<RuntimeEvalCaseResult>, String> {
    let cases = evaluation_cases_for_roles(model_id, roles, reasoning_effort)?;
    let mut results = Vec::with_capacity(cases.len());
    for case in cases {
        results.push(run_case(provider, case).await);
    }
    Ok(results)
}

/// Return stable case metadata without making provider calls.
///
/// # Errors
/// Returns an error when production-derived fixtures cannot be constructed.
pub fn runtime_eval_case_descriptors_for_roles(
    model_id: &str,
    roles: &[RuntimeEvalRole],
    reasoning_effort: Option<ReasoningEffort>,
) -> Result<Vec<RuntimeEvalCaseDescriptor>, String> {
    evaluation_cases_for_roles(model_id, roles, reasoning_effort).map(|cases| {
        cases
            .into_iter()
            .map(|case| RuntimeEvalCaseDescriptor {
                case_id: case.id.to_string(),
                role: case.role,
                category: case.category.to_string(),
                maximum_output_tokens: case.request.options.max_output_tokens.unwrap_or(0),
            })
            .collect()
    })
}

/// Run exactly one named case for checkpointed decision execution.
///
/// # Errors
/// Returns an error when fixtures cannot be constructed or the case is absent.
pub async fn run_runtime_case_for_roles(
    provider: &ProviderHandle,
    model_id: &str,
    roles: &[RuntimeEvalRole],
    reasoning_effort: Option<ReasoningEffort>,
    case_id: &str,
) -> Result<RuntimeEvalCaseResult, String> {
    let case = evaluation_cases_for_roles(model_id, roles, reasoning_effort)?
        .into_iter()
        .find(|case| case.id == case_id)
        .ok_or_else(|| format!("unknown runtime evaluation case: {case_id}"))?;
    Ok(run_case(provider, case).await)
}

async fn run_case(provider: &ProviderHandle, case: EvalCase) -> RuntimeEvalCaseResult {
    let started = Instant::now();
    let mut first_visible_delta = None;
    let mut streamed_text = String::new();
    let response = provider
        .generate_streaming(case.request, &mut |event| {
            if let GenerateStreamEvent::AssistantTextDelta { delta, .. } = event {
                first_visible_delta.get_or_insert_with(Instant::now);
                streamed_text.push_str(&delta);
            }
        })
        .await;
    let elapsed = started.elapsed();
    let first_visible_delta_ms =
        first_visible_delta.map(|first_visible| duration_ms(first_visible.duration_since(started)));
    let streamed_chars = streamed_text.chars().count();

    match response {
        Ok(response) => {
            let failure = grade_response(&case.expectation, &response, &streamed_text).err();
            let usage = response.usage.as_ref();
            RuntimeEvalCaseResult {
                case_id: case.id.to_string(),
                role: case.role,
                category: case.category.to_string(),
                critical: case.critical,
                passed: failure.is_none(),
                judge_rubric: case.expectation.judge_rubric().map(str::to_string),
                response_provider: Some(response.provider.clone()),
                response_model: Some(response.model.clone()),
                latency_ms: duration_ms(elapsed),
                first_visible_delta_ms,
                streamed_chars,
                input_tokens: usage.map(|usage| usage.input_tokens),
                cached_input_tokens: usage.and_then(|usage| usage.cached_input_tokens),
                output_tokens: usage.map(|usage| usage.output_tokens),
                assistant_text: bounded_text(&response.assistant_text(), 12_000),
                tool_calls: response
                    .tool_calls
                    .into_iter()
                    .map(|call| RuntimeEvalToolCall {
                        name: call.name,
                        payload: call.payload,
                    })
                    .collect(),
                failure,
            }
        }
        Err(error) => RuntimeEvalCaseResult {
            case_id: case.id.to_string(),
            role: case.role,
            category: case.category.to_string(),
            critical: case.critical,
            passed: false,
            judge_rubric: case.expectation.judge_rubric().map(str::to_string),
            response_provider: None,
            response_model: None,
            latency_ms: duration_ms(elapsed),
            first_visible_delta_ms,
            streamed_chars,
            input_tokens: None,
            cached_input_tokens: None,
            output_tokens: None,
            assistant_text: String::new(),
            tool_calls: Vec::new(),
            failure: Some(error.to_string()),
        },
    }
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn bounded_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}
