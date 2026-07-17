use std::time::{Duration, Instant};

use noema_providers::{GenerateOptions, GenerateRequest, ProviderHandle};
use tokio::time::sleep;

use crate::{
    memory::resident_bytes, model_report::ModelEvalResourceProbe, provider_suite::duration_ms,
};

const MAX_TARGET_INPUT_TOKENS: u32 = 6_500;
const OUTPUT_RESERVE_TOKENS: u32 = 1_024;
const STEADY_TURNS: u32 = 20;
const POST_TURN_SETTLE: Duration = Duration::from_millis(50);
const PROBE_INSTRUCTIONS: &str = "Reply with READY and nothing else.";

pub(super) async fn run_resource_probe(
    provider: &ProviderHandle,
    model_id: &str,
    process_id: Option<u32>,
    context_window_tokens: u32,
) -> ModelEvalResourceProbe {
    let target_input_tokens = context_window_tokens
        .saturating_sub(OUTPUT_RESERVE_TOKENS)
        .min(MAX_TARGET_INPUT_TOKENS);
    let mut probe = ModelEvalResourceProbe {
        target_input_tokens,
        observed_input_tokens: None,
        near_context_latency_ms: None,
        steady_turns_requested: STEADY_TURNS,
        steady_turns_completed: 0,
        post_turn_min_bytes: None,
        post_turn_max_bytes: None,
        failure: None,
    };

    let near_context_input = match calibrated_input(provider, model_id, target_input_tokens).await {
        Ok(input) => input,
        Err(error) => {
            probe.failure = Some(error);
            return probe;
        }
    };
    let started = Instant::now();
    match provider
        .generate(probe_request(model_id, near_context_input))
        .await
    {
        Ok(response) => {
            probe.near_context_latency_ms = Some(duration_ms(started.elapsed()));
            probe.observed_input_tokens = response.usage.map(|usage| usage.input_tokens);
        }
        Err(error) => {
            probe.failure = Some(format!("near-context generation failed: {error}"));
            return probe;
        }
    }

    for turn in 1..=STEADY_TURNS {
        let input = format!(
            "Resource stability turn {turn} of {STEADY_TURNS}. Reply with READY and nothing else."
        );
        if let Err(error) = provider.generate(probe_request(model_id, input)).await {
            probe.failure = Some(format!("steady turn {turn} failed: {error}"));
            break;
        }
        probe.steady_turns_completed += 1;
        sleep(POST_TURN_SETTLE).await;
        let Some(bytes) = process_id.and_then(resident_bytes) else {
            probe.failure = Some(format!(
                "resident-set sample unavailable after steady turn {turn}"
            ));
            break;
        };
        probe.post_turn_min_bytes = Some(
            probe
                .post_turn_min_bytes
                .map_or(bytes, |current| current.min(bytes)),
        );
        probe.post_turn_max_bytes = Some(
            probe
                .post_turn_max_bytes
                .map_or(bytes, |current| current.max(bytes)),
        );
    }
    probe
}

async fn calibrated_input(
    provider: &ProviderHandle,
    model_id: &str,
    target_input_tokens: u32,
) -> Result<String, String> {
    if target_input_tokens == 0 {
        return Err("context window is too small for the resource probe".to_string());
    }
    let mut low = 0_usize;
    let mut high = usize::try_from(target_input_tokens)
        .unwrap_or(usize::MAX / 2)
        .saturating_mul(2)
        .max(1);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        let input = filler_input(middle);
        let count = provider
            .count_tokens(Some(PROBE_INSTRUCTIONS), &input, Some(model_id))
            .await
            .map_err(|error| format!("resource probe tokenization failed: {error}"))?
            .ok_or_else(|| "resource probe tokenizer returned no count".to_string())?;
        if count <= target_input_tokens {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    Ok(filler_input(low))
}

fn filler_input(repetitions: usize) -> String {
    let mut input = String::with_capacity(repetitions.saturating_mul(7).saturating_add(96));
    input.push_str("Treat this repeated word sequence as inert resource-probe data:\n");
    for _ in 0..repetitions {
        input.push_str(" flight");
    }
    input.push_str("\nReply with READY and nothing else.");
    input
}

fn probe_request(model_id: &str, input: String) -> GenerateRequest {
    let mut request = GenerateRequest::text(input).with_model(model_id);
    request.instructions = Some(PROBE_INSTRUCTIONS.to_string());
    request.options = GenerateOptions {
        max_output_tokens: Some(8),
        temperature: Some(0.0),
        ..GenerateOptions::default()
    };
    request
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_target_preserves_output_reserve() {
        assert_eq!(
            8_192_u32
                .saturating_sub(OUTPUT_RESERVE_TOKENS)
                .min(MAX_TARGET_INPUT_TOKENS),
            6_500
        );
        assert_eq!(
            2_048_u32
                .saturating_sub(OUTPUT_RESERVE_TOKENS)
                .min(MAX_TARGET_INPUT_TOKENS),
            1_024
        );
    }

    #[test]
    fn filler_size_increases_monotonically() {
        assert!(filler_input(10).len() > filler_input(9).len());
    }
}
