use std::time::{Duration, Instant};

use noema_providers::{
    LocalModelEvalSession, LocalModelEvalSessionConfig, local_model_eval_runtime_version,
};

use crate::{
    memory::RuntimeMemorySampler,
    model_report::{ModelEvalConfig, ModelEvalReport},
    resource_probe::run_resource_probe,
};

pub(crate) async fn run_provider_suite(config: ModelEvalConfig) -> Result<ModelEvalReport, String> {
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

    let results =
        noema_runtime::eval_support::run_runtime_suite(&provider, &config.model_id).await?;
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
