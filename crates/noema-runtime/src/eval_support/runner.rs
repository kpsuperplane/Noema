use std::time::{Duration, Instant};

use noema_providers::{GenerateStreamEvent, ProviderHandle};

use super::{
    cases::evaluation_cases,
    grade::grade_response,
    types::{RuntimeEvalCaseResult, RuntimeEvalToolCall},
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
    let cases = evaluation_cases(model_id)?;
    let mut results = Vec::with_capacity(cases.len());
    for case in cases {
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
        let first_visible_delta_ms = first_visible_delta
            .map(|first_visible| duration_ms(first_visible.duration_since(started)));
        let streamed_chars = streamed_text.chars().count();

        let result = match response {
            Ok(response) => {
                let failure = grade_response(&case.expectation, &response, &streamed_text).err();
                let usage = response.usage.as_ref();
                RuntimeEvalCaseResult {
                    case_id: case.id.to_string(),
                    category: case.category.to_string(),
                    critical: case.critical,
                    passed: failure.is_none(),
                    latency_ms: duration_ms(elapsed),
                    first_visible_delta_ms,
                    streamed_chars,
                    input_tokens: usage.map(|usage| usage.input_tokens),
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
                category: case.category.to_string(),
                critical: case.critical,
                passed: false,
                latency_ms: duration_ms(elapsed),
                first_visible_delta_ms,
                streamed_chars,
                input_tokens: None,
                output_tokens: None,
                assistant_text: String::new(),
                tool_calls: Vec::new(),
                failure: Some(error.to_string()),
            },
        };
        results.push(result);
    }
    Ok(results)
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn bounded_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}
