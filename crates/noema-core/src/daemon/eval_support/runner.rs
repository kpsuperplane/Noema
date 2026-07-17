use std::time::{Duration, Instant};

use noema_providers::{
    GenerateStreamEvent, LocalModelEvalSession, LocalModelEvalSessionConfig,
    local_model_eval_runtime_version,
};

use super::{
    cases::evaluation_cases,
    grade::grade_response,
    memory::RuntimeMemorySampler,
    resource_probe::run_resource_probe,
    types::{ModelEvalCaseResult, ModelEvalConfig, ModelEvalReport, ModelEvalToolCall},
};

/// Run all deterministic direct-provider scenarios against one verified GGUF.
///
/// # Errors
///
/// Returns an error only when the evaluation configuration or production case
/// fixtures cannot be constructed. Runtime and per-case failures are retained
/// in the returned report so an incompatible model cannot abort a matrix run.
pub async fn run_provider_suite(config: ModelEvalConfig) -> Result<ModelEvalReport, String> {
    let version = local_model_eval_runtime_version();
    let load_started = Instant::now();
    let session = LocalModelEvalSession::start(LocalModelEvalSessionConfig {
        model_id: config.model_id.clone(),
        model_path: config.model_path,
        runtime_root: config.runtime_root,
        context_window_tokens: config.context_window_tokens,
        timeout_seconds: config.timeout_seconds,
        startup_timeout_seconds: config.startup_timeout_seconds,
    })
    .await;
    let runtime_load_ms = duration_ms(load_started.elapsed());
    let session = match session {
        Ok(session) => session,
        Err(error) => {
            return Ok(ModelEvalReport {
                model_id: config.model_id,
                llama_cpp_release: version.release_tag.to_string(),
                llama_cpp_commit: version.commit.to_string(),
                backend: None,
                runtime_load_ms,
                runtime_memory: None,
                resource_probe: None,
                runtime_error: Some(error.to_string()),
                cases: Vec::new(),
                passed_cases: 0,
                total_cases: 0,
                passed_critical_cases: 0,
                total_critical_cases: 0,
            });
        }
    };

    let provider = session.provider();
    let process_id = session.process_id();
    let memory_sampler = process_id.map(RuntimeMemorySampler::start);

    let cases = evaluation_cases(&config.model_id)?;
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
                ModelEvalCaseResult {
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
                        .map(|call| ModelEvalToolCall {
                            name: call.name,
                            payload: call.payload,
                        })
                        .collect(),
                    failure,
                }
            }
            Err(error) => ModelEvalCaseResult {
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
    let resource_probe = if config.run_resource_probe {
        Some(
            run_resource_probe(
                &provider,
                &config.model_id,
                process_id,
                config.context_window_tokens,
            )
            .await,
        )
    } else {
        None
    };
    let runtime_memory = match memory_sampler {
        Some(sampler) => sampler.finish().await,
        None => None,
    };
    session.shutdown().await;

    let total_cases = results.len();
    let passed_cases = results.iter().filter(|result| result.passed).count();
    let total_critical_cases = results.iter().filter(|result| result.critical).count();
    let passed_critical_cases = results
        .iter()
        .filter(|result| result.critical && result.passed)
        .count();
    Ok(ModelEvalReport {
        model_id: config.model_id,
        llama_cpp_release: version.release_tag.to_string(),
        llama_cpp_commit: version.commit.to_string(),
        backend: Some(session.selected_backend().display_name().to_string()),
        runtime_load_ms,
        runtime_memory,
        resource_probe,
        runtime_error: None,
        cases: results,
        passed_cases,
        total_cases,
        passed_critical_cases,
        total_critical_cases,
    })
}

pub(super) fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn bounded_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}
